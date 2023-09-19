use std::sync::Arc;

use anyhow::{anyhow, Result};
use noosphere_core::{
    authority::{Author, Authorization},
    context::{SphereContext, SphereContextKey, AUTHORIZATION, IDENTITY, USER_KEY_NAME},
    data::Did,
};
use noosphere_storage::KeyValueStore;

use crate::{
    key::KeyStorage,
    sphere::{generate_db, SphereContextBuilder, SphereContextBuilderArtifacts},
};

pub async fn open_a_sphere(
    builder: SphereContextBuilder,
    sphere_identity: Option<Did>,
) -> Result<SphereContextBuilderArtifacts> {
    let storage_path = builder.require_storage_path()?.to_owned();
    let db = generate_db(
        storage_path,
        builder.scoped_storage_layout,
        sphere_identity,
        builder.ipfs_gateway_url,
    )
    .await?;

    let user_key_name: String = db.require_key(USER_KEY_NAME).await?;
    let authorization = db.get_key(AUTHORIZATION).await?.map(Authorization::Cid);

    let author = match builder.key_storage {
        Some(key_storage) => {
            let key: SphereContextKey =
                Arc::new(Box::new(key_storage.require_key(&user_key_name).await?));

            Author { key, authorization }
        }
        _ => return Err(anyhow!("Unable to resolve sphere author")),
    };

    let sphere_identity = db.require_key(IDENTITY).await?;
    let mut context = SphereContext::new(sphere_identity, author, db, None).await?;

    if builder.gateway_api.is_some() {
        context
            .configure_gateway_url(builder.gateway_api.as_ref())
            .await?;
    }

    Ok(SphereContextBuilderArtifacts::SphereOpened(context))
}
