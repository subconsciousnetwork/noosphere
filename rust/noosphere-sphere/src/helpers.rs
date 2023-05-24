// #![cfg(any(test, doc, feature = "helpers"))]

use std::sync::Arc;

use anyhow::Result;
use noosphere_core::{
    authority::{generate_capability, generate_ed25519_key, Author, SphereAction},
    data::{Did, Link, LinkRecord},
    view::Sphere,
};
use noosphere_storage::{BlockStore, MemoryStorage, SphereDb, TrackingStorage, UcanStore};
use serde_json::json;
use tokio::sync::Mutex;
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

use crate::{walk_versioned_map_elements, walk_versioned_map_elements_and, SphereContext};

/// Access levels available when simulating a [SphereContext]
pub enum SimulationAccess {
    Readonly,
    ReadWrite,
}

/// Create a temporary, non-persisted [SphereContext] that tracks usage
/// internally. This is intended for use in docs and tests, and should otherwise
/// be ignored. When creating the simulated [SphereContext], you can pass a
/// [SimulationAccess] to control the kind of access the emphemeral credentials
/// have to the [SphereContext].
pub async fn simulated_sphere_context(
    profile: SimulationAccess,
    db: Option<SphereDb<TrackingStorage<MemoryStorage>>>,
) -> Result<Arc<Mutex<SphereContext<Ed25519KeyMaterial, TrackingStorage<MemoryStorage>>>>> {
    let mut db = match db {
        Some(db) => db,
        None => {
            let storage_provider = TrackingStorage::wrap(MemoryStorage::default());
            SphereDb::new(&storage_provider).await?
        }
    };

    let owner_key = generate_ed25519_key();
    let owner_did = owner_key.get_did().await?;

    let (sphere, proof, _) = Sphere::generate(&owner_did, &mut db).await?;

    let sphere_identity = sphere.get_identity().await?;
    let author = Author {
        key: owner_key,
        authorization: match profile {
            SimulationAccess::Readonly => None,
            SimulationAccess::ReadWrite => Some(proof),
        },
    };

    db.set_version(&sphere_identity, sphere.cid()).await?;

    Ok(Arc::new(Mutex::new(
        SphereContext::new(sphere_identity, author, db, None).await?,
    )))
}

/// Make a valid link record that represents a sphere "in the distance." The
/// link record and its proof are both put into the provided [UcanJwtStore]
pub async fn make_valid_link_record<S>(store: &mut S) -> Result<(Did, LinkRecord, Link<LinkRecord>)>
where
    S: UcanJwtStore,
{
    let owner_key = generate_ed25519_key();
    let owner_did = owner_key.get_did().await?;
    let mut db = SphereDb::new(&MemoryStorage::default()).await?;

    let (sphere, proof, _) = Sphere::generate(&owner_did, &mut db).await?;
    let ucan_proof = proof.resolve_ucan(&db).await?;

    let sphere_identity = sphere.get_identity().await?;

    let link_record = LinkRecord::from(
        UcanBuilder::default()
            .issued_by(&owner_key)
            .for_audience(&sphere_identity)
            .witnessed_by(&ucan_proof)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAction::Publish,
            ))
            .with_lifetime(120)
            .with_fact(json!({
              "link": sphere.cid().to_string()
            }))
            .build()?
            .sign()
            .await?,
    );

    store.write_token(&ucan_proof.encode()?).await?;
    let link = Link::from(store.write_token(&link_record.encode()?).await?);

    Ok((sphere_identity, link_record, link))
}

#[cfg(docs)]
use noosphere_core::data::MemoIpld;

/// Attempt to walk an entire sphere, touching every block up to and including
/// any [MemoIpld] nodes, but excluding those memo's body content. This helper
/// is useful for asserting that the blocks expected to be sent during
/// replication have in fact been sent.
pub async fn touch_all_sphere_blocks<S>(sphere: &Sphere<S>) -> Result<()>
where
    S: BlockStore + 'static,
{
    trace!("Touching content blocks...");
    let content = sphere.get_content().await?;
    let _ = content.load_changelog().await?;

    walk_versioned_map_elements(content).await?;

    trace!("Touching identity blocks...");
    let identities = sphere.get_address_book().await?.get_identities().await?;
    let _ = identities.load_changelog().await?;

    walk_versioned_map_elements_and(
        identities,
        sphere.store().clone(),
        |_, identity, store| async move {
            let ucan_store = UcanStore(store);
            if let Some(record) = identity.link_record(&ucan_store).await {
                record.collect_proofs(&ucan_store).await?;
            }
            Ok(())
        },
    )
    .await?;

    trace!("Touching authority blocks...");
    let authority = sphere.get_authority().await?;

    trace!("Touching delegation blocks...");
    let delegations = authority.get_delegations().await?;
    walk_versioned_map_elements(delegations).await?;

    trace!("Touching revocation blocks...");
    let revocations = authority.get_revocations().await?;
    walk_versioned_map_elements(revocations).await?;

    Ok(())
}
