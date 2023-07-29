use std::{collections::BTreeMap, marker::PhantomData};

use anyhow::{anyhow, Result};
use noosphere_api::data::{FetchParameters, PushBody, PushResponse};
use noosphere_core::{
    authority::{generate_capability, SphereAbility},
    data::{Did, IdentityIpld, Jwt, Link, MemoIpld, LINK_RECORD_FACT_NAME},
    view::{Sphere, Timeline},
};
use noosphere_storage::{KeyValueStore, SphereDb, Storage};
use tokio_stream::StreamExt;
use ucan::builder::UcanBuilder;

use crate::{
    metadata::COUNTERPART, HasMutableSphereContext, SpherePetnameRead, SpherePetnameWrite,
    SyncError,
};

type HandshakeResults = (Option<Link<MemoIpld>>, Did, Option<Link<MemoIpld>>);
type FetchResults = (
    Link<MemoIpld>,
    Link<MemoIpld>,
    BTreeMap<String, IdentityIpld>,
);
type CounterpartHistory<S> = Vec<Result<(Link<MemoIpld>, Sphere<SphereDb<S>>)>>;

/// The default synchronization strategy is a git-like fetch->rebase->push flow.
/// It depends on the corresponding history of a "counterpart" sphere that is
/// owned by a gateway server. As revisions are pushed to the gateway server, it
/// updates its own sphere to point to the tip of the latest lineage of the
/// user's. When a new change needs to be synchronized, the latest history of
/// the counterpart sphere is first fetched, and the local changes are rebased
/// on the counterpart sphere's reckoning of the authoritative lineage of the
/// user's sphere. Finally, after the rebase, the reconciled local lineage is
/// pushed to the gateway.
pub struct GatewaySyncStrategy<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    has_context_type: PhantomData<C>,
    store_type: PhantomData<S>,
}

impl<C, S> Default for GatewaySyncStrategy<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    fn default() -> Self {
        Self {
            has_context_type: Default::default(),
            store_type: Default::default(),
        }
    }
}

impl<C, S> GatewaySyncStrategy<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Synchronize a local sphere's data with the data in a gateway, and rollback
    /// if there is an error. The returned [Link] is the latest version of the local
    /// sphere lineage after the sync has completed.
    pub async fn sync(&self, context: &mut C) -> Result<Link<MemoIpld>, SyncError>
    where
        C: HasMutableSphereContext<S>,
    {
        let (local_sphere_version, counterpart_sphere_identity, counterpart_sphere_version) =
            self.handshake(context).await?;

        let result: Result<Link<MemoIpld>, anyhow::Error> = {
            let (mut local_sphere_version, counterpart_sphere_version, updated_names) = self
                .fetch_remote_changes(
                    context,
                    local_sphere_version.as_ref(),
                    &counterpart_sphere_identity,
                    counterpart_sphere_version.as_ref(),
                )
                .await?;

            if let Some(version) = self.adopt_names(context, updated_names).await? {
                local_sphere_version = version;
            }

            self.push_local_changes(
                context,
                &local_sphere_version,
                &counterpart_sphere_identity,
                &counterpart_sphere_version,
            )
            .await?;

            Ok(local_sphere_version)
        };

        // Rollback if there is an error while syncing
        if result.is_err() {
            self.rollback(
                context,
                local_sphere_version.as_ref(),
                &counterpart_sphere_identity,
                counterpart_sphere_version.as_ref(),
            )
            .await?
        }

        Ok(result?)
    }

    #[instrument(level = "debug", skip(self, context))]
    async fn handshake(&self, context: &mut C) -> Result<HandshakeResults> {
        let mut context = context.sphere_context_mut().await?;
        let client = context.client().await?;
        let counterpart_sphere_identity = client.session.sphere_identity.clone();

        // TODO: Some kind of due diligence to notify the caller when this value
        // changes
        context
            .db_mut()
            .set_key(COUNTERPART, &counterpart_sphere_identity)
            .await?;

        let local_sphere_identity = context.identity().clone();

        let local_sphere_version = context.db().get_version(&local_sphere_identity).await?;
        let counterpart_sphere_version = context
            .db()
            .get_version(&counterpart_sphere_identity)
            .await?;

        Ok((
            local_sphere_version.map(|cid| cid.into()),
            counterpart_sphere_identity,
            counterpart_sphere_version.map(|cid| cid.into()),
        ))
    }

    /// Fetches the latest changes from a gateway and updates the local lineage
    /// using a conflict-free rebase strategy
    #[instrument(level = "debug", skip(self, context))]
    async fn fetch_remote_changes(
        &self,
        context: &mut C,
        local_sphere_tip: Option<&Link<MemoIpld>>,
        counterpart_sphere_identity: &Did,
        counterpart_sphere_base: Option<&Link<MemoIpld>>,
    ) -> Result<FetchResults> {
        let mut context = context.sphere_context_mut().await?;
        let local_sphere_identity = context.identity().clone();
        let client = context.client().await?;

        let fetch_response = client
            .fetch(&FetchParameters {
                since: counterpart_sphere_base.cloned(),
            })
            .await?;

        let mut updated_names = BTreeMap::new();

        let (counterpart_sphere_tip, block_stream) = match fetch_response {
            Some((tip, stream)) => (tip, stream),
            None => {
                info!("Local history is already up to date...");
                let local_sphere_tip = context
                    .db()
                    .require_version(&local_sphere_identity)
                    .await?
                    .into();
                return Ok((
                    local_sphere_tip,
                    counterpart_sphere_base
                        .ok_or_else(|| anyhow!("Counterpart sphere history is missing!"))?
                        .clone(),
                    updated_names,
                ));
            }
        };

        context.db_mut().put_block_stream(block_stream).await?;

        trace!("Finished putting block stream");

        let counterpart_history: CounterpartHistory<S> =
            Sphere::at(&counterpart_sphere_tip, context.db_mut())
                .into_history_stream(counterpart_sphere_base)
                .collect()
                .await;

        trace!("Iterating over counterpart history");

        for item in counterpart_history.into_iter().rev() {
            let (_, sphere) = item?;
            sphere.hydrate().await?;
            updated_names.append(
                &mut sphere
                    .get_address_book()
                    .await?
                    .get_identities()
                    .await?
                    .get_added()
                    .await?,
            );
        }

        let local_sphere_old_base = match counterpart_sphere_base {
            Some(counterpart_sphere_base) => Sphere::at(counterpart_sphere_base, context.db())
                .get_content()
                .await?
                .get(&local_sphere_identity)
                .await?
                .cloned(),
            None => None,
        };
        let local_sphere_new_base = Sphere::at(&counterpart_sphere_tip, context.db())
            .get_content()
            .await?
            .get(&local_sphere_identity)
            .await?
            .cloned();

        let local_sphere_tip = match (
            local_sphere_tip,
            local_sphere_old_base,
            local_sphere_new_base,
        ) {
            // History diverged, so rebase our local changes on the newly received branch
            (Some(current_tip), Some(old_base), Some(new_base)) if old_base != new_base => {
                info!(
                    ?current_tip,
                    ?old_base,
                    ?new_base,
                    "Syncing received local sphere revisions..."
                );
                Sphere::at(current_tip, context.db())
                    .rebase(
                        &old_base,
                        &new_base,
                        &context.author().key,
                        context.author().authorization.as_ref(),
                    )
                    .await?
            }
            // No diverged history, just new linear history based on our local tip
            (None, old_base, Some(new_base)) => {
                info!("Hydrating received local sphere revisions...");
                let timeline = Timeline::new(context.db_mut());
                Sphere::hydrate_timeslice(
                    &timeline.slice(&new_base, old_base.as_ref()).exclude_past(),
                )
                .await?;

                new_base.clone()
            }
            // No new history at all
            (Some(current_tip), _, _) => {
                info!("Nothing to sync!");
                current_tip.clone()
            }
            // We should have local history but we don't!
            _ => {
                return Err(anyhow!("Missing local history for sphere after sync!"));
            }
        };

        context
            .db_mut()
            .set_version(&local_sphere_identity, &local_sphere_tip)
            .await?;

        debug!("Setting counterpart sphere version to {counterpart_sphere_tip}");

        context
            .db_mut()
            .set_version(counterpart_sphere_identity, &counterpart_sphere_tip)
            .await?;

        Ok((local_sphere_tip, counterpart_sphere_tip, updated_names))
    }

    #[instrument(level = "debug", skip(self, context))]
    async fn adopt_names(
        &self,
        context: &mut C,
        updated_names: BTreeMap<String, IdentityIpld>,
    ) -> Result<Option<Link<MemoIpld>>> {
        if updated_names.is_empty() {
            return Ok(None);
        }
        info!(
            "Considering {} updated link records for adoption...",
            updated_names.len()
        );

        let db = context.sphere_context().await?.db().clone();

        for (name, address) in updated_names.into_iter() {
            if let Some(link_record) = address.link_record(&db).await {
                if let Some(identity) = context.get_petname(&name).await? {
                    if identity != address.did {
                        warn!("Updated link record for {name} referred to unexpected sphere; expected {identity}, but record referred to {}; skipping...", address.did);
                        continue;
                    }

                    if context.resolve_petname(&name).await? == link_record.get_link() {
                        // TODO: Should probably also verify record expiry in case we are dealing
                        // with a renewed record to the same link
                        warn!("Resolved link for {name} has not changed; skipping...");
                        continue;
                    }

                    context.set_petname_record(&name, &link_record).await?;
                } else {
                    debug!("Not adopting link record for {name}, which is no longer present in the address book")
                }
            }
        }

        Ok(if context.has_unsaved_changes().await? {
            Some(context.save(None).await?)
        } else {
            None
        })
    }

    /// Attempts to push the latest local lineage to the gateway, causing the
    /// gateway to update its own pointer to the tip of the local sphere's history
    #[instrument(level = "debug", skip(self, context))]
    async fn push_local_changes(
        &self,
        context: &mut C,
        local_sphere_tip: &Link<MemoIpld>,
        counterpart_sphere_identity: &Did,
        counterpart_sphere_tip: &Link<MemoIpld>,
    ) -> Result<(), SyncError> {
        let mut context = context.sphere_context_mut().await?;

        let local_sphere_base = Sphere::at(counterpart_sphere_tip, context.db())
            .get_content()
            .await?
            .get(context.identity())
            .await?
            .cloned();

        if local_sphere_base.as_ref() == Some(local_sphere_tip) {
            info!("Gateway is already up to date!");
            return Ok(());
        }

        info!("Collecting blocks from new local history...");

        let bundle = Sphere::at(local_sphere_tip, context.db())
            .bundle_until_ancestor(local_sphere_base.as_ref())
            .await?;

        let client = context.client().await?;

        let local_sphere_identity = context.identity();
        let authorization = context
            .author()
            .require_authorization()?
            .as_ucan(context.db())
            .await?;

        let name_record = Jwt(UcanBuilder::default()
            .issued_by(&context.author().key)
            .for_audience(local_sphere_identity)
            .witnessed_by(&authorization, None)
            .claiming_capability(&generate_capability(
                local_sphere_identity,
                SphereAbility::Publish,
            ))
            .with_lifetime(120)
            .with_fact(LINK_RECORD_FACT_NAME, local_sphere_tip.to_string())
            .build()?
            .sign()
            .await?
            .encode()?);

        info!(
            "Pushing new local history to gateway {}...",
            client.session.gateway_identity
        );

        let result = client
            .push(&PushBody {
                sphere: local_sphere_identity.clone(),
                local_base: local_sphere_base,
                local_tip: local_sphere_tip.clone(),
                counterpart_tip: Some(counterpart_sphere_tip.clone()),
                blocks: bundle,
                name_record: Some(name_record),
            })
            .await?;

        let (counterpart_sphere_updated_tip, new_blocks) = match result {
            PushResponse::Accepted { new_tip, blocks } => (new_tip, blocks),
            PushResponse::NoChange => {
                return Err(SyncError::Other(anyhow!("Gateway already up to date!")));
            }
        };

        info!("Saving updated counterpart sphere history...");

        new_blocks.load_into(context.db_mut()).await?;

        debug!(
            "Hydrating updated counterpart sphere history (from {} back to {})...",
            counterpart_sphere_tip, counterpart_sphere_updated_tip
        );

        let timeline = Timeline::new(context.db_mut());
        Sphere::hydrate_timeslice(
            &timeline
                .slice(
                    &counterpart_sphere_updated_tip,
                    Some(counterpart_sphere_tip),
                )
                .exclude_past(),
        )
        .await?;

        context
            .db_mut()
            .set_version(counterpart_sphere_identity, &counterpart_sphere_updated_tip)
            .await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self, context))]
    async fn rollback(
        &self,
        context: &mut C,
        original_sphere_version: Option<&Link<MemoIpld>>,
        counterpart_identity: &Did,
        original_counterpart_version: Option<&Link<MemoIpld>>,
    ) -> Result<()> {
        let sphere_identity = context.identity().await?;
        let mut context = context.sphere_context_mut().await?;

        if let Some(version) = original_sphere_version {
            context
                .db_mut()
                .set_version(&sphere_identity, version)
                .await?;
        }

        if let Some(version) = original_counterpart_version {
            context
                .db_mut()
                .set_version(counterpart_identity, version)
                .await?;
        }

        Ok(())
    }
}
