use std::sync::Arc;

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    authority::{generate_capability, generate_ed25519_key, Author, Authorization, SphereAbility},
    context::{
        SphereAuthorityRead, SphereContext, SphereSync, SyncExtent, SyncRecovery, AUTHORIZATION,
        GATEWAY_URL, IDENTITY, USER_KEY_NAME,
    },
    data::Did,
};
use noosphere_storage::KeyValueStore;
use tokio::sync::Mutex;
use ucan::{builder::UcanBuilder, crypto::KeyMaterial};

use crate::{
    key::KeyStorage,
    sphere::{generate_db, SphereContextBuilder, SphereContextBuilderArtifacts},
};

pub async fn recover_a_sphere(
    builder: SphereContextBuilder,
    sphere_identity: Did,
) -> Result<SphereContextBuilderArtifacts> {
    // 1. Check to make sure the local key is available

    let key_storage = builder.require_key_storage()?;
    let local_key_name = builder.require_key_name()?.to_owned();
    let local_key_did = Did(key_storage
        .require_key(&local_key_name)
        .await?
        .get_did()
        .await?);

    // 2. Backup existing database, moving it to a new location
    // NOTE: Database backups not available on wasm targets

    let storage_path = builder.require_storage_path()?.to_owned();

    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::storage::StorageLayout;
        use noosphere_storage::BackupStorage;
        use std::path::PathBuf;

        let storage_layout: StorageLayout = (
            storage_path.clone(),
            builder.scoped_storage_layout,
            Some(sphere_identity.clone()),
        )
            .try_into()?;

        let database_root = PathBuf::from(storage_layout);

        debug!(?database_root);

        if database_root.exists() {
            info!("Backing up existing database...");
            crate::platform::PrimitiveStorage::backup(&database_root).await?;
        }
    }

    // 3. Generate a new, one-time authorization with the mnemonic

    let one_time_key: Arc<Box<dyn KeyMaterial>> = Arc::new(Box::new(generate_ed25519_key()));
    let one_time_key_identity = one_time_key.get_did().await?;
    let root_key = builder.require_mnemonic()?.to_credential()?;

    let authorization = Authorization::Ucan(
        UcanBuilder::default()
            .issued_by(&root_key)
            .for_audience(&one_time_key_identity)
            .claiming_capability(&generate_capability(&sphere_identity, SphereAbility::Fetch))
            .with_lifetime(60 * 60)
            .with_nonce()
            .build()?
            .sign()
            .await?,
    );

    // 4. Generate a new DB for the sphere

    let mut db = generate_db(
        storage_path,
        builder.scoped_storage_layout,
        Some(sphere_identity.clone()),
        builder.ipfs_gateway_url.clone(),
    )
    .await?;

    // 5. Attempt to sync (fetch-only) from the gateway

    db.set_key(IDENTITY, &sphere_identity).await?;
    db.set_key(GATEWAY_URL, builder.require_gateway_api()?)
        .await?;

    let author = Author {
        key: one_time_key,
        authorization: Some(authorization),
    };

    let mut context = Arc::new(Mutex::new(
        SphereContext::new(sphere_identity.clone(), author, db.clone(), None).await?,
    ));
    context
        .sync_with_options(SyncExtent::FetchOnly, SyncRecovery::None)
        .await?;

    // TODO: Should probably revoke the authorization of the one-time
    // key at the end for good measure.

    // 6. Recover the original authorization

    let authorization = context
        .get_authorization(&local_key_did)
        .await?
        .ok_or_else(|| anyhow!("No authorization for key '{}' found!", local_key_name))?;

    db.set_key(USER_KEY_NAME, local_key_name.to_owned()).await?;
    db.set_key(AUTHORIZATION, Cid::try_from(&authorization)?)
        .await?;

    // 7. Initialize the recovered sphere using the intended author

    let local_key: Arc<Box<dyn KeyMaterial>> =
        Arc::new(Box::new(key_storage.require_key(&local_key_name).await?));
    let author = Author {
        key: local_key,
        authorization: Some(authorization),
    };

    let context = SphereContext::new(sphere_identity, author, db, None).await?;

    Ok(SphereContextBuilderArtifacts::SphereOpened(context))
}
