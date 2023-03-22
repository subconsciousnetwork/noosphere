use std::{collections::BTreeSet, marker::PhantomData};

use anyhow::Result;

use axum::{http::StatusCode, Extension};

use cid::Cid;
use noosphere_api::data::{PushBody, PushError, PushResponse};
use noosphere_core::{
    authority::{SphereAction, SphereReference},
    data::{Bundle, MapOperation},
    view::Sphere,
};
use noosphere_sphere::{HasMutableSphereContext, SphereContentWrite, SphereCursor};
use noosphere_storage::Storage;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;
use ucan::capability::{Capability, Resource, With};
use ucan::crypto::KeyMaterial;

use crate::{
    authority::GatewayAuthority,
    extractor::Cbor,
    worker::{NameSystemJob, SyndicationJob},
    GatewayScope,
};

// #[debug_handler]
pub async fn push_route<C, K, S>(
    authority: GatewayAuthority<K>,
    Extension(sphere_context): Extension<C>,
    Extension(gateway_scope): Extension<GatewayScope>,
    Extension(syndication_tx): Extension<UnboundedSender<SyndicationJob<C>>>,
    Extension(name_system_tx): Extension<UnboundedSender<NameSystemJob<C>>>,
    Cbor(request_body): Cbor<PushBody>,
) -> Result<Cbor<PushResponse>, StatusCode>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone,
    S: Storage + 'static,
{
    debug!("Invoking push route...");

    let sphere_identity = &request_body.sphere;

    if sphere_identity != &gateway_scope.counterpart {
        return Err(StatusCode::FORBIDDEN);
    }

    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: gateway_scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Push,
    })?;

    let gateway_push_routine = GatewayPushRoutine {
        sphere_context,
        gateway_scope,
        syndication_tx,
        name_system_tx,
        request_body,
        key_type: PhantomData,
        storage_type: PhantomData,
    };

    Ok(Cbor(gateway_push_routine.invoke().await?))
}

pub struct GatewayPushRoutine<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    sphere_context: C,
    gateway_scope: GatewayScope,
    syndication_tx: UnboundedSender<SyndicationJob<C>>,
    name_system_tx: UnboundedSender<NameSystemJob<C>>,
    request_body: PushBody,
    key_type: PhantomData<K>,
    storage_type: PhantomData<S>,
}

impl<C, K, S> GatewayPushRoutine<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    pub async fn invoke(mut self) -> Result<PushResponse, PushError> {
        debug!("Invoking gateway push...");

        self.verify_history().await?;
        self.incorporate_history().await?;
        self.synchronize_names().await?;
        let (next_version, new_blocks) = self.update_gateway_sphere().await?;

        // These steps are order-independent
        let _ = tokio::join!(
            self.notify_ipfs_syndicator(next_version),
            self.notify_name_resolver(),
        );

        Ok(PushResponse::Accepted {
            new_tip: next_version,
            blocks: new_blocks,
        })
    }

    /// Ensure that the pushed history is not in direct conflict with our
    /// history (in which case the pusher typically must sync first), and that
    /// there is no missing history implied by the pushed history (in which
    /// case, there is probably a sync bug in the client implementation).
    async fn verify_history(&self) -> Result<(), PushError> {
        debug!("Verifying pushed sphere history...");

        let sphere_identity = &self.request_body.sphere;
        let gateway_sphere_context = self.sphere_context.sphere_context().await?;
        let db = gateway_sphere_context.db();

        let local_sphere_base_cid = db.get_version(sphere_identity).await?;
        let request_sphere_base_cid = self.request_body.base;

        match (local_sphere_base_cid, request_sphere_base_cid) {
            (Some(mine), theirs) => {
                // TODO(#26): Probably should do some diligence here to check if
                // their base is even in our lineage. Note that this condition
                // will be hit if theirs is ahead of mine, which actually
                // should be a "missing revisions" condition.
                let conflict = match theirs {
                    Some(cid) if cid != mine => true,
                    None => true,
                    _ => false,
                };

                if conflict {
                    warn!("Conflict!");
                    return Err(PushError::Conflict);
                }

                if self.request_body.tip == mine {
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

        Ok(())
    }

    /// Incorporate the pushed history into our storage, hydrating each new
    /// revision in the history as we go. Then, update our local pointer to the
    /// tip of the pushed history.
    async fn incorporate_history(&mut self) -> Result<(), PushError> {
        {
            debug!("Merging pushed sphere history...");
            let mut sphere_context = self.sphere_context.sphere_context_mut().await?;

            self.request_body
                .blocks
                .load_into(sphere_context.db_mut())
                .await?;

            let PushBody { base, tip, .. } = &self.request_body;

            let history: Vec<Result<(Cid, Sphere<_>)>> = Sphere::at(tip, sphere_context.db())
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
            .link_raw(&self.gateway_scope.counterpart, &self.request_body.tip)
            .await?;

        Ok(())
    }

    async fn synchronize_names(&mut self) -> Result<(), PushError> {
        debug!("Synchronizing name changes to local sphere...");

        let my_sphere = self.sphere_context.to_sphere().await?;
        let my_names = my_sphere.get_address_book().await?.get_identities().await?;

        let sphere = Sphere::at(&self.request_body.tip, my_sphere.store());
        let stream = sphere.into_history_stream(self.request_body.base.as_ref());

        tokio::pin!(stream);

        let mut updated_names = BTreeSet::<String>::new();

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
                        if updated_names.contains(&key) {
                            trace!("Skipping name add for {}...", key);
                            continue;
                        }

                        let my_value = my_names.get(&key).await?;

                        // Only add to the mutation if the value has actually
                        // changed to avoid redundantly recording updates made
                        // on the client due to a previous sync
                        if my_value != Some(&value) {
                            debug!("Adding name {}...", key);
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
                        if updated_names.contains(&key) {
                            trace!("Skipping name removal for {}...", key);
                            continue;
                        }

                        debug!("Removing name {}...", key);
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
    async fn update_gateway_sphere(&mut self) -> Result<(Cid, Bundle), PushError> {
        debug!("Updating the gateway's sphere...");

        let previous_version = self.sphere_context.version().await?;
        let next_version = SphereCursor::latest(self.sphere_context.clone())
            .save(None)
            .await?;

        let blocks = self
            .sphere_context
            .to_sphere()
            .await?
            .bundle_until_ancestor(Some(&previous_version))
            .await?;

        Ok((next_version, blocks))
    }

    /// Notify the name system that new names may need to be resolved
    async fn notify_name_resolver(&self) -> Result<()> {
        if let Some(name_record) = &self.request_body.name_record {
            if let Err(error) = self.name_system_tx.send(NameSystemJob::Publish {
                context: self.sphere_context.clone(),
                record: name_record.clone(),
            }) {
                warn!("Failed to request name record publish: {}", error);
            }
        }

        if let Err(error) = self.name_system_tx.send(NameSystemJob::ResolveSince {
            context: self.sphere_context.clone(),
            since: self.request_body.base,
        }) {
            warn!("Failed to request name system resolutions: {}", error);
        };

        Ok(())
    }

    /// Request that new history be syndicated to IPFS
    async fn notify_ipfs_syndicator(&self, next_version: Cid) -> Result<()> {
        // TODO(#156): This should not be happening on every push, but rather on
        // an explicit publish action. Move this to the publish handler when we
        // have added it to the gateway.
        if let Err(error) = self.syndication_tx.send(SyndicationJob {
            revision: next_version,
            context: self.sphere_context.clone(),
        }) {
            warn!("Failed to queue IPFS syndication job: {}", error);
        };

        Ok(())
    }
}
