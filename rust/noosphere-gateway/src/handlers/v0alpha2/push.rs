use std::{collections::BTreeSet, marker::PhantomData};

use anyhow::Result;

use async_stream::try_stream;
use axum::{body::StreamBody, extract::BodyStream, Extension};

use bytes::Bytes;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_core::api::v0alpha2::{PushBody, PushError, PushResponse};
use noosphere_core::context::{HasMutableSphereContext, SphereContentWrite, SphereCursor};
use noosphere_core::stream::{
    from_car_stream, memo_history_stream, put_block_stream, to_car_stream,
};
use noosphere_core::{
    authority::{generate_capability, SphereAbility},
    data::{Link, LinkRecord, MapOperation, MemoIpld},
    view::Sphere,
};
use noosphere_storage::{block_deserialize, block_serialize, Storage};
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::{Stream, StreamExt};

use crate::worker::SyndicationJobIroh;
use crate::{
    authority::GatewayAuthority,
    error::GatewayErrorResponse,
    worker::{NameSystemJob, SyndicationJob},
    GatewayScope,
};

// #[debug_handler]
#[instrument(
    level = "debug",
    skip(
        authority,
        gateway_scope,
        sphere_context,
        syndication_tx,
        name_system_tx,
        stream
    )
)]
pub async fn push_route<C, S>(
    authority: GatewayAuthority<S>,
    Extension(sphere_context): Extension<C>,
    Extension(gateway_scope): Extension<GatewayScope>,
    Extension(syndication_tx): Extension<UnboundedSender<SyndicationJobIroh<C>>>,
    Extension(name_system_tx): Extension<UnboundedSender<NameSystemJob<C>>>,
    stream: BodyStream,
) -> Result<StreamBody<impl Stream<Item = Result<Bytes, std::io::Error>>>, GatewayErrorResponse>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Invoking push route...");

    authority.try_authorize(&generate_capability(
        &gateway_scope.counterpart,
        SphereAbility::Push,
    ))?;

    let gateway_push_routine = GatewayPushRoutine {
        sphere_context,
        gateway_scope,
        syndication_tx,
        name_system_tx,
        block_stream: Box::pin(from_car_stream(stream)),
        storage_type: PhantomData,
    };

    Ok(StreamBody::new(gateway_push_routine.invoke().await?))
}

pub struct GatewayPushRoutine<C, S, St>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
    St: Stream<Item = Result<(Cid, Vec<u8>)>> + Unpin + 'static,
{
    sphere_context: C,
    gateway_scope: GatewayScope,
    syndication_tx: UnboundedSender<SyndicationJobIroh<C>>,
    name_system_tx: UnboundedSender<NameSystemJob<C>>,
    block_stream: St,
    storage_type: PhantomData<S>,
}

impl<C, S, St> GatewayPushRoutine<C, S, St>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
    St: Stream<Item = Result<(Cid, Vec<u8>)>> + Unpin + 'static,
{
    #[instrument(level = "debug", skip(self))]
    pub async fn invoke(
        mut self,
    ) -> Result<impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static, PushError> {
        debug!("Invoking gateway push...");

        let push_body = self.verify_history().await?;

        debug!(?push_body, "Received valid push body...");

        self.incorporate_history(&push_body).await?;
        self.synchronize_names(&push_body).await?;

        let (next_version, new_blocks) = self.update_gateway_sphere().await?;

        // These steps are order-independent
        let _ = tokio::join!(
            self.notify_name_resolver(&push_body),
            self.notify_ipfs_syndicator(next_version)
        );

        let roots = vec![next_version.into()];

        let block_stream = try_stream! {
            yield block_serialize::<DagCborCodec, _>(PushResponse::Accepted {
                new_tip: next_version
            })?;

            for await block in new_blocks {
                yield block?;
            }
        };

        Ok(to_car_stream(roots, block_stream))
    }

    /// Ensure that the pushed history is not in direct conflict with our
    /// history (in which case the pusher typically must sync first), and that
    /// there is no missing history implied by the pushed history (in which
    /// case, there is probably a sync bug in the client implementation).
    async fn verify_history(&mut self) -> Result<PushBody, PushError> {
        debug!("Verifying pushed sphere history...");

        let push_body = if let Some((_, first_block)) = self.block_stream.try_next().await? {
            block_deserialize::<DagCborCodec, PushBody>(&first_block)?
        } else {
            return Err(PushError::UnexpectedBody);
        };

        let gateway_sphere_tip = self.sphere_context.version().await?;
        if Some(&gateway_sphere_tip) != push_body.counterpart_tip.as_ref() {
            warn!(
                "Gateway sphere conflict; we have {gateway_sphere_tip}, they have {:?}",
                push_body.counterpart_tip
            );
            return Err(PushError::Conflict);
        }

        let sphere_identity = &push_body.sphere;
        let gateway_sphere_context = self.sphere_context.sphere_context().await?;
        let db = gateway_sphere_context.db();

        let local_sphere_base_cid = db.get_version(sphere_identity).await?.map(|cid| cid.into());
        let request_sphere_base_cid = push_body.local_base;

        match (local_sphere_base_cid, request_sphere_base_cid) {
            (Some(mine), theirs) => {
                // TODO(#26): Probably should do some diligence here to check if
                // their base is even in our lineage. Note that this condition
                // will be hit if theirs is ahead of mine, which actually
                // should be a "missing revisions" condition.
                let conflict = match &theirs {
                    Some(cid) if cid != &mine => true,
                    None => true,
                    _ => false,
                };

                if conflict {
                    warn!(
                        "Counterpart sphere conflict; we have {mine}, they have {:?}",
                        theirs
                    );
                    return Err(PushError::Conflict);
                }

                if push_body.local_tip == mine {
                    warn!("No new changes in push body!");
                    return Err(PushError::UpToDate);
                }
            }
            (None, Some(_)) => {
                error!("Missing local lineage!");
                return Err(PushError::MissingHistory);
            }
            _ => (),
        };

        Ok(push_body)
    }

    /// Incorporate the pushed history into our storage, hydrating each new
    /// revision in the history as we go. Then, update our local pointer to the
    /// tip of the pushed history.
    async fn incorporate_history(&mut self, push_body: &PushBody) -> Result<(), PushError> {
        {
            debug!("Merging pushed sphere history...");
            let mut sphere_context = self.sphere_context.sphere_context_mut().await?;

            put_block_stream(sphere_context.db_mut().clone(), &mut self.block_stream).await?;

            let PushBody {
                local_base: base,
                local_tip: tip,
                ..
            } = &push_body;

            let history: Vec<Result<(Link<MemoIpld>, Sphere<_>)>> =
                Sphere::at(tip, sphere_context.db())
                    .into_history_stream(base.as_ref())
                    .collect()
                    .await;

            for step in history.into_iter().rev() {
                let (cid, sphere) = step?;
                debug!("Hydrating {}", cid);
                sphere.hydrate().await?;
            }

            debug!(
                "Setting {} tip to {}...",
                self.gateway_scope.counterpart, tip
            );

            sphere_context
                .db_mut()
                .set_version(&self.gateway_scope.counterpart, tip)
                .await?;
        }

        self.sphere_context
            .link_raw(&self.gateway_scope.counterpart, &push_body.local_tip)
            .await?;

        Ok(())
    }

    async fn synchronize_names(&mut self, push_body: &PushBody) -> Result<(), PushError> {
        debug!("Synchronizing name changes to local sphere...");

        let my_sphere = self.sphere_context.to_sphere().await?;
        let my_names = my_sphere.get_address_book().await?.get_identities().await?;

        let sphere = Sphere::at(&push_body.local_tip, my_sphere.store());
        let stream = sphere.into_history_stream(push_body.local_base.as_ref());

        tokio::pin!(stream);

        let mut updated_names = BTreeSet::<String>::new();
        let mut removed_names = BTreeSet::<String>::new();

        // Walk backwards through the history of the pushed sphere and aggregate
        // name changes into a single mutation
        while let Ok(Some((_, sphere))) = stream.try_next().await {
            let changed_names = sphere
                .get_address_book()
                .await?
                .get_identities()
                .await?
                .load_changelog()
                .await?;
            for operation in changed_names.changes {
                match operation {
                    MapOperation::Add { key, value } => {
                        // Since we are walking backwards through history, we
                        // can ignore changes to names in the past that we have
                        // already encountered in the future
                        if updated_names.contains(&key) || removed_names.contains(&key) {
                            trace!("Skipping name add for '{}' (already seen)...", key);
                            continue;
                        }

                        let my_value = my_names.get(&key).await?;

                        // Only add to the mutation if the value has actually
                        // changed to avoid redundantly recording updates made
                        // on the client due to a previous sync
                        if my_value != Some(&value) {
                            debug!("Adding name '{}' ({})...", key, value.did);
                            self.sphere_context
                                .sphere_context_mut()
                                .await?
                                .mutation_mut()
                                .identities_mut()
                                .set(&key, &value);
                        }

                        updated_names.insert(key);
                    }
                    MapOperation::Remove { key } => {
                        removed_names.insert(key.clone());

                        if updated_names.contains(&key) {
                            trace!("Skipping name removal for '{}' (already seen)...", key);
                            continue;
                        }

                        debug!("Removing name '{}'...", key);
                        self.sphere_context
                            .sphere_context_mut()
                            .await?
                            .mutation_mut()
                            .identities_mut()
                            .remove(&key);

                        updated_names.insert(key);
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply any mutations accrued during the push operation to the local
    /// sphere and return the new version, along with the blocks needed to
    /// synchronize the pusher with the latest local history.
    async fn update_gateway_sphere(
        &mut self,
    ) -> Result<(Link<MemoIpld>, impl Stream<Item = Result<(Cid, Vec<u8>)>>), PushError> {
        debug!("Updating the gateway's sphere...");

        // NOTE CDATA: "Previous version" doesn't cover all cases; this needs to be a version given
        // in the push body, or else we don't know how far back we actually have to go (e.g., the name
        // system may have created a new version in the mean time.
        let previous_version = self.sphere_context.version().await?;
        let next_version = SphereCursor::latest(self.sphere_context.clone())
            .save(None)
            .await?;

        let db = self.sphere_context.sphere_context().await?.db().clone();
        let block_stream = memo_history_stream(db, &next_version, Some(&previous_version), false);

        Ok((next_version, block_stream))
    }

    /// Notify the name system that new names may need to be resolved
    async fn notify_name_resolver(&self, push_body: &PushBody) -> Result<()> {
        if let Some(name_record) = &push_body.name_record {
            if let Err(error) = self.name_system_tx.send(NameSystemJob::Publish {
                context: self.sphere_context.clone(),
                record: LinkRecord::try_from(name_record)?,
                republish: false,
            }) {
                warn!("Failed to request name record publish: {}", error);
            }
        }

        if let Err(error) = self.name_system_tx.send(NameSystemJob::ResolveSince {
            context: self.sphere_context.clone(),
            since: push_body.local_base,
        }) {
            warn!("Failed to request name system resolutions: {}", error);
        };

        Ok(())
    }

    /// Request that new history be syndicated to IPFS
    async fn notify_ipfs_syndicator(&self, next_version: Link<MemoIpld>) -> Result<()> {
        // TODO(#156): This should not be happening on every push, but rather on
        // an explicit publish action. Move this to the publish handler when we
        // have added it to the gateway.
        if let Err(error) = self.syndication_tx.send(SyndicationJobIroh {
            revision: next_version,
            context: self.sphere_context.clone(),
        }) {
            warn!("Failed to queue IPFS syndication job: {}", error);
        };

        Ok(())
    }
}
