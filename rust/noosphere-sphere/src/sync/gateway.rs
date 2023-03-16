use std::{collections::BTreeMap, marker::PhantomData};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_api::data::{FetchParameters, FetchResponse, PushBody, PushResponse};
use noosphere_core::{
    authority::{SphereAction, SphereReference},
    data::{AddressIpld, Did, Jwt},
    view::Sphere,
};
use noosphere_storage::{KeyValueStore, SphereDb, Storage};
use serde_json::json;
use tokio_stream::StreamExt;
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
};

use crate::{metadata::COUNTERPART, HasMutableSphereContext, SpherePetnameWrite};

/// The default synchronization strategy is a git-like fetch->rebase->push flow.
/// It depends on the corresponding history of a "counterpart" sphere that is
/// owned by a gateway server. As revisions are pushed to the gateway server, it
/// updates its own sphere to point to the tip of the latest lineage of the
/// user's. When a new change needs to be synchronized, the latest history of
/// the counterpart sphere is first fetched, and the local changes are rebased
/// on the counterpart sphere's reckoning of the authoritative lineage of the
/// user's sphere. Finally, after the rebase, the reconciled local lineage is
/// pushed to the gateway.
pub struct GatewaySyncStrategy<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    has_context_type: PhantomData<C>,
    key_type: PhantomData<K>,
    store_type: PhantomData<S>,
}

impl<C, K, S> Default for GatewaySyncStrategy<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    fn default() -> Self {
        Self {
            has_context_type: Default::default(),
            key_type: Default::default(),
            store_type: Default::default(),
        }
    }
}

impl<C, K, S> GatewaySyncStrategy<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Synchronize a local sphere's data with the data in a gateway, and rollback
    /// if there is an error.
    pub async fn sync(&self, context: &mut C) -> Result<()>
    where
        C: HasMutableSphereContext<K, S>,
    {
        let (local_sphere_version, counterpart_sphere_identity, counterpart_sphere_version) =
            self.handshake(context).await?;

        let result: Result<(), anyhow::Error> = {
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
            Ok(())
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

        result
    }

    async fn handshake(&self, context: &mut C) -> Result<(Option<Cid>, Did, Option<Cid>)> {
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
            local_sphere_version,
            counterpart_sphere_identity,
            counterpart_sphere_version,
        ))
    }

    /// Fetches the latest changes from a gateway and updates the local lineage
    /// using a conflict-free rebase strategy
    async fn fetch_remote_changes(
        &self,
        context: &mut C,
        local_sphere_tip: Option<&Cid>,
        counterpart_sphere_identity: &Did,
        counterpart_sphere_base: Option<&Cid>,
    ) -> Result<(Cid, Cid, BTreeMap<String, AddressIpld>)> {
        let mut context = context.sphere_context_mut().await?;
        let local_sphere_identity = context.identity().clone();
        let client = context.client().await?;
        let fetch_response = client
            .fetch(&FetchParameters {
                since: counterpart_sphere_base.cloned(),
            })
            .await?;
        let mut updated_names = BTreeMap::new();

        let (counterpart_sphere_tip, new_blocks) = match fetch_response {
            FetchResponse::NewChanges { tip, blocks } => (tip, blocks),
            FetchResponse::UpToDate => {
                println!("Local history is already up to date...");
                let local_sphere_tip = context.db().require_version(&local_sphere_identity).await?;
                return Ok((
                    local_sphere_tip,
                    *counterpart_sphere_base
                        .ok_or_else(|| anyhow!("Counterpart sphere history is missing!"))?,
                    updated_names,
                ));
            }
        };

        new_blocks.load_into(context.db_mut()).await?;

        let counterpart_history: Vec<Result<(Cid, Sphere<SphereDb<S>>)>> =
            Sphere::at(&counterpart_sphere_tip, context.db_mut())
                .into_history_stream(counterpart_sphere_base)
                .collect()
                .await;

        for item in counterpart_history.into_iter().rev() {
            let (_, sphere) = item?;
            sphere.hydrate().await?;
            updated_names.append(&mut sphere.get_names().await?.get_added().await?);
        }

        let local_sphere_old_base = match counterpart_sphere_base {
            Some(counterpart_sphere_base) => {
                Sphere::at(counterpart_sphere_base, context.db())
                    .get_links()
                    .await?
                    .get_as_cid::<DagCborCodec>(&local_sphere_identity)
                    .await?
            }
            None => None,
        };
        let local_sphere_new_base = Sphere::at(&counterpart_sphere_tip, context.db())
            .get_links()
            .await?
            .get_as_cid::<DagCborCodec>(&local_sphere_identity)
            .await?;

        let local_sphere_tip = match (
            local_sphere_tip,
            local_sphere_old_base,
            local_sphere_new_base,
        ) {
            // History diverged, so rebase our local changes on the newly received branch
            (Some(current_tip), Some(old_base), Some(new_base)) => {
                println!("Syncing received local sphere revisions...");
                let new_tip = Sphere::at(current_tip, context.db())
                    .sync(
                        &old_base,
                        &new_base,
                        &context.author().key,
                        context.author().authorization.as_ref(),
                    )
                    .await?;

                new_tip
            }
            // No diverged history, just new linear history based on our local tip
            (None, old_base, Some(new_base)) => {
                println!("Hydrating received local sphere revisions...");
                Sphere::hydrate_range(old_base.as_ref(), &new_base, context.db_mut()).await?;

                new_base
            }
            // No new history at all
            (Some(current_tip), _, _) => {
                println!("Nothing to sync!");
                *current_tip
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

    async fn adopt_names(
        &self,
        context: &mut C,
        updated_names: BTreeMap<String, AddressIpld>,
    ) -> Result<Option<Cid>> {
        if updated_names.is_empty() {
            return Ok(None);
        }
        info!(
            "Adopting {} updated name resolutions...",
            updated_names.len()
        );

        let db = context.sphere_context().await?.db().clone();

        for (name, address) in updated_names.into_iter() {
            if let Some(jwt) = address.get_proof(&db).await {
                context.adopt_petname(&name, &jwt).await?;
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
    async fn push_local_changes(
        &self,
        context: &mut C,
        local_sphere_tip: &Cid,
        counterpart_sphere_identity: &Did,
        counterpart_sphere_tip: &Cid,
    ) -> Result<()> {
        let mut context = context.sphere_context_mut().await?;

        let local_sphere_base = Sphere::at(counterpart_sphere_tip, context.db())
            .get_links()
            .await?
            .get_as_cid::<DagCborCodec>(context.identity())
            .await?;

        if local_sphere_base.as_ref() == Some(local_sphere_tip) {
            println!("Gateway is already up to date!");
            return Ok(());
        }

        println!("Collecting blocks from new local history...");

        let bundle = Sphere::at(local_sphere_tip, context.db())
            .bundle_until_ancestor(local_sphere_base.as_ref())
            .await?;

        let client = context.client().await?;

        let local_sphere_identity = context.identity();
        let authorization = context
            .author()
            .require_authorization()?
            .resolve_ucan(context.db())
            .await?;

        let name_record = Jwt(UcanBuilder::default()
            .issued_by(&context.author().key)
            .for_audience(local_sphere_identity)
            .witnessed_by(&authorization)
            .claiming_capability(&Capability {
                with: With::Resource {
                    kind: Resource::Scoped(SphereReference {
                        did: local_sphere_identity.to_string(),
                    }),
                },
                can: SphereAction::Publish,
            })
            .with_lifetime(120)
            .with_fact(json!({
              "link": local_sphere_tip.to_string()
            }))
            .build()?
            .sign()
            .await?
            .encode()?);

        println!(
            "Pushing new local history to gateway {}...",
            client.session.gateway_identity
        );

        let result = client
            .push(&PushBody {
                sphere: local_sphere_identity.clone(),
                base: local_sphere_base,
                tip: *local_sphere_tip,
                blocks: bundle,
                name_record: Some(name_record),
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

        Sphere::hydrate_range(
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
        context: &mut C,
        original_sphere_version: Option<&Cid>,
        counterpart_identity: &Did,
        original_counterpart_version: Option<&Cid>,
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
