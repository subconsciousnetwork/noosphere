use std::sync::Arc;

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    api::v0alpha1::ReplicateParameters,
    authority::{generate_capability, generate_ed25519_key, Author, Authorization, SphereAbility},
    context::{
        HasMutableSphereContext, HasSphereContext, SphereAuthorityRead, SphereContentRead,
        SphereContext, SphereCursor, AUTHORIZATION, GATEWAY_URL, IDENTITY, USER_KEY_NAME,
    },
    data::Did,
    stream::put_block_stream,
};
use noosphere_storage::KeyValueStore;
use noosphere_ucan::{builder::UcanBuilder, crypto::KeyMaterial};
use tokio::sync::Mutex;

use crate::{
    key::KeyStorage,
    sphere::{generate_db, SphereContextBuilder, SphereContextBuilderArtifacts},
};

pub async fn recover_a_sphere(
    builder: SphereContextBuilder,
    user_sphere_identity: Did,
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
        use rand::Rng;
        use std::path::PathBuf;

        let storage_layout: StorageLayout = (
            storage_path.clone(),
            builder.scoped_storage_layout,
            Some(user_sphere_identity.clone()),
        )
            .try_into()?;

        let database_root: PathBuf = storage_layout.into();

        debug!(?database_root);

        if database_root.exists() {
            let timestamp = std::time::SystemTime::UNIX_EPOCH.elapsed()?;
            let nonce = rand::thread_rng().gen::<u32>();
            let backup_id = format!("backup.{}-{}", timestamp.as_nanos(), nonce);

            let mut backup_root = database_root.clone();
            backup_root.set_extension(backup_id);

            info!("Backing up existing database...");

            tokio::fs::rename(database_root, backup_root).await?;
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
            .claiming_capability(&generate_capability(
                &user_sphere_identity,
                SphereAbility::Fetch,
            ))
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
        Some(user_sphere_identity.clone()),
        builder.ipfs_gateway_url.clone(),
        builder.storage_config.clone(),
    )
    .await?;

    // 5. Replicate the gateway's sphere, including its content (which
    // incidentally has a slug pointing to our sphere)

    db.set_key(IDENTITY, &user_sphere_identity).await?;
    db.set_key(GATEWAY_URL, builder.require_gateway_api()?)
        .await?;

    let author = Author {
        key: one_time_key,
        authorization: Some(authorization),
    };

    let mut context = Arc::new(Mutex::new(
        SphereContext::new(user_sphere_identity.clone(), author, db.clone(), None).await?,
    ));

    let client = context.sphere_context_mut().await?.client().await?;

    let gateway_sphere_identity = client.session.sphere_identity.clone();
    let (gateway_root, stream) = client
        .replicate(
            gateway_sphere_identity.clone(),
            Some(&ReplicateParameters {
                since: None,
                include_content: true,
            }),
        )
        .await?;
    put_block_stream(db.clone(), stream).await?;
    db.set_version(&gateway_sphere_identity, &gateway_root)
        .await?;

    let gateway_sphere_context = SphereCursor::latest(Arc::new(
        SphereContext::new(
            gateway_sphere_identity.clone(),
            context.sphere_context().await?.author().clone(),
            db.clone(),
            None,
        )
        .await?,
    ));

    let sphere_version = gateway_sphere_context
        .read(&user_sphere_identity)
        .await?
        .ok_or_else(|| anyhow!("Gateway pointer to user sphere not found"))?
        .memo_version;

    db.set_version(&user_sphere_identity, &sphere_version)
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

    let context = SphereContext::new(user_sphere_identity, author, db, None).await?;

    Ok(SphereContextBuilderArtifacts::SphereOpened(context))
}
