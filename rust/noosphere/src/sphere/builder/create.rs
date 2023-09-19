use std::sync::Arc;

use crate::{
    key::KeyStorage,
    sphere::{generate_db, SphereContextBuilder, SphereContextBuilderArtifacts},
};
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    authority::Author,
    context::{SphereContext, SphereContextKey, AUTHORIZATION, IDENTITY, USER_KEY_NAME},
    view::Sphere,
};
use noosphere_storage::{KeyValueStore, MemoryStore};

pub async fn create_a_sphere(
    builder: SphereContextBuilder,
) -> Result<SphereContextBuilderArtifacts> {
    let storage_path = builder.require_storage_path()?.to_owned();
    let key_storage = builder
        .key_storage
        .as_ref()
        .ok_or_else(|| anyhow!("No key storage configured!"))?;
    let key_name = builder
        .key_name
        .as_ref()
        .ok_or_else(|| anyhow!("No key name configured!"))?;
    if builder.authorization.is_some() {
        warn!("Creating a new sphere; the configured authorization will be ignored!");
    }

    let owner_key: SphereContextKey = Arc::new(Box::new(key_storage.require_key(key_name).await?));
    let owner_did = owner_key.get_did().await?;

    // NOTE: We generate the sphere in-memory because we don't know where to
    // store it on disk until we have its ID
    let mut memory_store = MemoryStore::default();
    let (sphere, authorization, mnemonic) = Sphere::generate(&owner_did, &mut memory_store)
        .await
        .unwrap();

    let sphere_did = sphere.get_identity().await.unwrap();
    let mut db = generate_db(
        storage_path,
        builder.scoped_storage_layout,
        Some(sphere_did.clone()),
        builder.ipfs_gateway_url,
    )
    .await?;

    db.persist(&memory_store).await?;

    db.set_version(&sphere_did, sphere.cid()).await?;

    db.set_key(IDENTITY, &sphere_did).await?;
    db.set_key(USER_KEY_NAME, key_name.to_owned()).await?;
    db.set_key(AUTHORIZATION, Cid::try_from(&authorization)?)
        .await?;

    let mut context = SphereContext::new(
        sphere_did,
        Author {
            key: owner_key,
            authorization: Some(authorization),
        },
        db,
        None,
    )
    .await?;

    if builder.gateway_api.is_some() {
        context
            .configure_gateway_url(builder.gateway_api.as_ref())
            .await?;
    }

    Ok(SphereContextBuilderArtifacts::SphereCreated { context, mnemonic })
}
