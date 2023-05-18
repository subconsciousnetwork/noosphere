// #![cfg(any(test, doc, feature = "helpers"))]

use std::sync::Arc;

use anyhow::Result;
use noosphere_core::{
    authority::{generate_capability, generate_ed25519_key, Author, SphereAction},
    data::{Did, Link, LinkRecord},
    view::Sphere,
};
use noosphere_storage::{MemoryStorage, SphereDb, TrackingStorage};
use serde_json::json;
use tokio::sync::Mutex;
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

use crate::SphereContext;

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

    let sphere_identity = sphere.get_identity().await.unwrap();
    let author = Author {
        key: owner_key,
        authorization: match profile {
            SimulationAccess::Readonly => None,
            SimulationAccess::ReadWrite => Some(proof),
        },
    };

    db.set_version(&sphere_identity, sphere.cid()).await?;

    Ok(Arc::new(Mutex::new(
        SphereContext::new(sphere_identity, author, db, None)
            .await
            .unwrap(),
    )))
}

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

    let link = Link::from(store.write_token(&link_record.encode()?).await?);

    Ok((sphere_identity, link_record, link))
}
