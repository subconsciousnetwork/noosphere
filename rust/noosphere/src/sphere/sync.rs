use std::marker::PhantomData;

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_api::data::{FetchParameters, FetchResponse, PushBody, PushResponse};
use noosphere_core::{data::Did, view::Sphere};
use noosphere_storage::{db::SphereDb, interface::Store};
use ucan::crypto::KeyMaterial;

use super::SphereContext;

/// The default synchronization strategy is a git-like fetch->rebase->push flow.
/// It depends on the corresponding history of a "counterpart" sphere that is
/// owned by a gateway server. As revisions are pushed to the gateway server, it
/// updates its own sphere to point to the tip of the latest lineage of the
/// user's. When a new change needs to be synchronized, the latest history of
/// the counterpart sphere is first fetched, and the local changes are rebased
/// on the counterpart sphere's reckoning of the authoritative lineage of the
/// user's sphere. Finally, after the rebase, the reconciled local lineage is
/// pushed to the gateway.
pub struct GatewaySyncStrategy<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Store,
{
    key_type: PhantomData<K>,
    store_type: PhantomData<S>,
}

impl<K, S> Default for GatewaySyncStrategy<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Store,
{
    fn default() -> Self {
        Self {
            key_type: Default::default(),
            store_type: Default::default(),
        }
    }
}

impl<K, S> GatewaySyncStrategy<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Store,
{
    /// Synchronize a local sphere's data with the data in a gateway, and rollback
    /// if there is an error.
    pub async fn sync(&self, context: &mut SphereContext<K, S>) -> Result<()> {
        let client = context.client().await?;
        let counterpart_sphere_identity = client.session.sphere_identity.clone();
        let local_sphere_identity = context.identity().clone();

        let local_sphere_version = context.db().get_version(&local_sphere_identity).await?;
        let counterpart_sphere_version = context
            .db()
            .get_version(&counterpart_sphere_identity)
            .await?;

        let result: Result<(), anyhow::Error> = {
            let (local_sphere_version, counterpart_sphere_version) = self
                .fetch_remote_changes(
                    context,
                    local_sphere_version.as_ref(),
                    &counterpart_sphere_identity,
                    counterpart_sphere_version.as_ref(),
                )
                .await?;
            self.push_local_changes(
                context,
                local_sphere_version.as_ref(),
                &counterpart_sphere_identity,
                &counterpart_sphere_version,
            )
            .await?;
            Ok(())
        };

        // Rollback if there is an error while syncing
        if result.is_err() {
            self.rollback(
                context.db_mut(),
                &local_sphere_identity,
                local_sphere_version.as_ref(),
                &counterpart_sphere_identity,
                counterpart_sphere_version.as_ref(),
            )
            .await?
        }

        result
    }

    /// Fetches the latest changes from a gateway and updates the local lineage
    /// using a conflict-free rebase strategy
    async fn fetch_remote_changes(
        &self,
        context: &mut SphereContext<K, S>,
        local_sphere_tip: Option<&Cid>,
        counterpart_sphere_identity: &Did,
        counterpart_sphere_base: Option<&Cid>,
    ) -> Result<(Option<Cid>, Cid)> {
        let local_sphere_identity = context.identity().clone();
        let client = context.client().await?;
        let fetch_response = client
            .fetch(&FetchParameters {
                since: counterpart_sphere_base.cloned(),
            })
            .await?;

        let (counterpart_sphere_tip, new_blocks) = match fetch_response {
            FetchResponse::NewChanges { tip, blocks } => (tip, blocks),
            FetchResponse::UpToDate => {
                println!("Local history is already up to date...");
                return Ok((
                    local_sphere_tip.cloned(),
                    counterpart_sphere_base
                        .ok_or_else(|| anyhow!("Counterpart sphere history is missing!"))?
                        .clone(),
                ));
            }
        };

        new_blocks.load_into(context.db_mut()).await?;

        Sphere::try_hydrate_range(
            counterpart_sphere_base,
            &counterpart_sphere_tip,
            context.db_mut(),
        )
        .await?;

        let local_sphere_old_base = match counterpart_sphere_base {
            Some(counterpart_sphere_base) => Sphere::at(counterpart_sphere_base, context.db())
                .try_get_links()
                .await?
                .get(&local_sphere_identity)
                .await?
                .cloned(),
            None => None,
        };
        let local_sphere_new_base = Sphere::at(&counterpart_sphere_tip, context.db())
            .try_get_links()
            .await?
            .get(&local_sphere_identity)
            .await?
            .cloned();

        let local_sphere_tip = match (
            local_sphere_tip,
            local_sphere_old_base,
            local_sphere_new_base,
        ) {
            (Some(current_tip), Some(old_base), Some(new_base)) => {
                println!("Syncing received local sphere revisions...");
                let new_tip = Sphere::at(current_tip, context.db())
                    .try_sync(
                        &old_base,
                        &new_base,
                        &context.author().key,
                        context.author().authorization.as_ref(),
                    )
                    .await?;

                context
                    .db_mut()
                    .set_version(&local_sphere_identity, &new_tip)
                    .await?;

                Some(new_tip)
            }
            (None, old_base, Some(new_base)) => {
                println!("Hydrating received local sphere revisions...");
                Sphere::try_hydrate_range(old_base.as_ref(), &new_base, context.db_mut()).await?;

                context
                    .db_mut()
                    .set_version(&local_sphere_identity, &new_base)
                    .await?;

                None
            }
            _ => {
                println!("Nothing to sync!");
                local_sphere_tip.cloned()
            }
        };

        debug!("Setting counterpart sphere version to {counterpart_sphere_tip}");
        context
            .db_mut()
            .set_version(counterpart_sphere_identity, &counterpart_sphere_tip)
            .await?;

        Ok((local_sphere_tip, counterpart_sphere_tip))
    }

    /// Attempts to push the latest local lineage to the gateway, causing the
    /// gateway to update its own pointer to the tip of the local sphere's history
    async fn push_local_changes(
        &self,
        context: &mut SphereContext<K, S>,
        local_sphere_tip: Option<&Cid>,
        counterpart_sphere_identity: &Did,
        counterpart_sphere_tip: &Cid,
    ) -> Result<()> {
        // The base of the changes that must be pushed is the tip of our lineage as
        // recorded by the most recent history of the gateway's sphere. Everything
        // past that point in history represents new changes that the gateway does
        // not yet know about.
        let local_sphere_tip = match local_sphere_tip {
            Some(cid) => cid,
            None => {
                println!("No local history for local sphere {}!", context.identity());
                return Ok(());
            }
        };

        let local_sphere_base = Sphere::at(counterpart_sphere_tip, context.db())
            .try_get_links()
            .await?
            .get(context.identity())
            .await?
            .cloned();

        if local_sphere_base.as_ref() == Some(local_sphere_tip) {
            println!("Gateway is already up to date!");
            return Ok(());
        }

        println!("Collecting blocks from new local history...");

        let bundle = Sphere::at(local_sphere_tip, context.db())
            .try_bundle_until_ancestor(local_sphere_base.as_ref())
            .await?;

        let client = context.client().await?;

        println!(
            "Pushing new local history to gateway {}...",
            client.session.gateway_identity
        );

        let result = client
            .push(&PushBody {
                sphere: context.identity().to_string(),
                base: local_sphere_base,
                tip: *local_sphere_tip,
                blocks: bundle,
            })
            .await?;

        let (counterpart_sphere_updated_tip, new_blocks) = match result {
            PushResponse::Accepted { new_tip, blocks } => (new_tip, blocks),
            PushResponse::NoChange => {
                return Err(anyhow!("Gateway already up to date!"));
            }
        };

        println!("Saving updated counterpart sphere history...");

        new_blocks.load_into(context.db_mut()).await?;

        Sphere::try_hydrate_range(
            Some(counterpart_sphere_tip),
            &counterpart_sphere_updated_tip,
            context.db_mut(),
        )
        .await?;

        context
            .db_mut()
            .set_version(counterpart_sphere_identity, &counterpart_sphere_updated_tip)
            .await?;

        Ok(())
    }

    async fn rollback(
        &self,
        db: &mut SphereDb<S>,
        sphere_identity: &Did,
        original_sphere_version: Option<&Cid>,
        counterpart_identity: &Did,
        original_counterpart_version: Option<&Cid>,
    ) -> Result<()> {
        if let Some(version) = original_sphere_version {
            db.set_version(sphere_identity, version).await?;
        }

        if let Some(version) = original_counterpart_version {
            db.set_version(counterpart_identity, version).await?;
        }

        Ok(())
    }
}
