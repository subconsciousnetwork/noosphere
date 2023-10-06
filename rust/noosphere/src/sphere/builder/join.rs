use std::sync::Arc;

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    authority::Author,
    context::{SphereContext, SphereContextKey, AUTHORIZATION, IDENTITY, USER_KEY_NAME},
    data::Did,
};
use noosphere_storage::KeyValueStore;

use crate::{
    key::KeyStorage,
    sphere::{generate_db, SphereContextBuilder, SphereContextBuilderArtifacts},
};

pub async fn join_a_sphere(
    builder: SphereContextBuilder,
    sphere_identity: Did,
) -> Result<SphereContextBuilderArtifacts> {
    let key_storage = builder
        .key_storage
        .as_ref()
        .ok_or_else(|| anyhow!("No key storage configured!"))?;
    let key_name = builder
        .key_name
        .as_ref()
        .ok_or_else(|| anyhow!("No key name configured!"))?;
    let storage_path = builder.require_storage_path()?.to_owned();

    let user_key: SphereContextKey = Arc::new(Box::new(key_storage.require_key(key_name).await?));

    let mut db = generate_db(
        storage_path,
        builder.scoped_storage_layout,
        Some(sphere_identity.clone()),
        builder.ipfs_gateway_url.clone(),
        builder.storage_config.clone(),
    )
    .await?;

    db.set_key(IDENTITY, &sphere_identity).await?;
    db.set_key(USER_KEY_NAME, key_name.to_owned()).await?;

    if let Some(authorization) = &builder.authorization {
        db.set_key(AUTHORIZATION, Cid::try_from(authorization)?)
            .await?;
    }

    debug!("Initializing context...");

    let mut context = SphereContext::new(
        sphere_identity,
        Author {
            key: user_key,
            authorization: builder.authorization.clone(),
        },
        db,
        None,
    )
    .await?;

    debug!("Configuring gateway URL...");

    if builder.gateway_api.is_some() {
        context
            .configure_gateway_url(builder.gateway_api.as_ref())
            .await?;
    }

    Ok(SphereContextBuilderArtifacts::SphereOpened(context))
}
