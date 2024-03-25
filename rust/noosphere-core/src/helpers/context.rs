//! These helpers are intended for use in documentation examples and tests only.
//! They are useful for quickly scaffolding common scenarios that would
//! otherwise be verbosely rubber-stamped in a bunch of places.
use std::{sync::Arc, time::Duration};

use crate::{
    authority::{generate_ed25519_key, Access, Author},
    context::SphereReplicaWrite,
    data::{ContentType, Mnemonic},
    view::Sphere,
};
use anyhow::Result;
use noosphere_storage::{BlockStore, MemoryStorage, SphereDb, Storage, TrackingStorage, UcanStore};
use noosphere_ucan::crypto::KeyMaterial;
use tokio::{io::AsyncReadExt, sync::Mutex};

use crate::{
    context::{
        HasMutableSphereContext, HasSphereContext, SphereContentRead, SphereContentWrite,
        SphereContext, SphereContextKey, SpherePetnameWrite,
    },
    stream::{walk_versioned_map_elements, walk_versioned_map_elements_and},
};

/// An alias for the [HasMutableSphereContext] type returned by [simulated_sphere_context]
pub type SimulatedHasMutableSphereContext =
    Arc<Mutex<SphereContext<TrackingStorage<MemoryStorage>>>>;

/// Create a temporary, non-persisted [SphereContext] that tracks usage
/// internally. This is intended for use in docs and tests, and should otherwise
/// be ignored. When creating the simulated [SphereContext], you can pass an
/// [Access] to control the kind of access the emphemeral credentials
/// have to the [SphereContext].
pub async fn simulated_sphere_context(
    profile: Access,
    db: Option<SphereDb<TrackingStorage<MemoryStorage>>>,
) -> Result<(SimulatedHasMutableSphereContext, Mnemonic)> {
    let db = match db {
        Some(db) => db,
        None => {
            let storage_provider = TrackingStorage::wrap(MemoryStorage::default());
            SphereDb::new(&storage_provider).await?
        }
    };

    generate_sphere_context(profile, db).await
}

/// Generate a [SphereContext] using the storage provided, intended for tests and
/// benchmarks. You can pass a [Access] to control access.
pub async fn generate_sphere_context<S: Storage + 'static>(
    profile: Access,
    mut db: SphereDb<S>,
) -> Result<(Arc<Mutex<SphereContext<S>>>, Mnemonic)> {
    let owner_key: SphereContextKey = Arc::new(Box::new(generate_ed25519_key()));
    let owner_did = owner_key.get_did().await?;

    let (sphere, proof, mnemonic) = Sphere::generate(&owner_did, &mut db).await?;

    let sphere_identity = sphere.get_identity().await?;
    let author = Author {
        key: owner_key,
        authorization: match profile {
            Access::ReadOnly => None,
            Access::ReadWrite => Some(proof),
        },
    };

    db.set_version(&sphere_identity, sphere.cid()).await?;

    Ok((
        Arc::new(Mutex::new(
            SphereContext::new(sphere_identity, author, db, None).await?,
        )),
        mnemonic,
    ))
}

#[cfg(docs)]
use crate::data::MemoIpld;

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

/// A type of [HasMutableSphereContext] that uses [TrackingStorage] internally
pub type TrackedHasMutableSphereContext = Arc<Mutex<SphereContext<TrackingStorage<MemoryStorage>>>>;

/// Create a series of spheres where each sphere has the next as resolved entry
/// in its address book; return a [HasMutableSphereContext] for the first sphere
/// in the sequence. The returned [HasMutableSphereContext] will have a peer in
/// its address book that refers to the first link in the caller-prescribed
/// chain of peers, so if you suggest a chain of three peers (for example), a
/// total of four sphere contexts will be scaffolded.
pub async fn make_sphere_context_with_peer_chain(
    peer_chain: &[String],
) -> Result<(
    TrackedHasMutableSphereContext,
    Vec<TrackedHasMutableSphereContext>,
)> {
    let (origin_sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
        .await
        .unwrap();

    let mut db = origin_sphere_context
        .sphere_context()
        .await
        .unwrap()
        .db()
        .clone();

    let mut contexts = vec![origin_sphere_context.clone()];

    for name in peer_chain.iter() {
        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, Some(db.clone()))
            .await
            .unwrap();

        sphere_context
            .write("my-name", &ContentType::Subtext, name.as_bytes(), None)
            .await
            .unwrap();
        sphere_context.save(None).await.unwrap();

        contexts.push(sphere_context);
    }

    let mut next_sphere_context: Option<TrackedHasMutableSphereContext> = None;
    let mut sphere_contexts = Vec::new();

    for mut sphere_context in contexts.into_iter().rev() {
        if let Some(mut next_sphere_context) = next_sphere_context {
            sphere_contexts.push(next_sphere_context.clone());
            let link_record = next_sphere_context
                .create_link_record(Some(Duration::from_secs(120)))
                .await?;

            let mut name = String::new();
            let mut file = next_sphere_context.read("my-name").await.unwrap().unwrap();
            file.contents.read_to_string(&mut name).await.unwrap();

            debug!("Adopting {name}");
            sphere_context
                .set_petname(&name, Some(next_sphere_context.identity().await?))
                .await?;
            sphere_context.save(None).await?;

            sphere_context
                .set_petname_record(&name, &link_record)
                .await
                .unwrap();

            sphere_context.save(None).await?;
        }

        db.set_version(
            &sphere_context.identity().await?,
            &sphere_context.version().await?.into(),
        )
        .await
        .unwrap();

        next_sphere_context = Some(sphere_context);
    }

    sphere_contexts.reverse();

    Ok((origin_sphere_context, sphere_contexts))
}
