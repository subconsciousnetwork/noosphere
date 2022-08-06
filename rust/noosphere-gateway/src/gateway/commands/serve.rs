use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use async_std::sync::Mutex;
use axum::{
    http::HeaderValue,
    routing::{get, put},
    Extension, Router, Server,
};
use hyper::Method;
use noosphere::authority::SUPPORTED_KEYS;
use noosphere_storage::{
    interface::{StorageProvider, Store},
    native::{NativeStorageInit, NativeStorageProvider},
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use ucan::crypto::did::DidParser;
use url::Url;

use crate::gateway::{
    environment::{Blocks, GatewayConfig, GatewayRoot, GatewayState},
    handlers::{fetch_handler, identify_handler, push_handler},
};

pub const GATEWAY_STATE_STORE: &str = "gateway_state";
pub const BLOCK_STORE: &str = "blocks";

pub async fn serve<Storage, Provider>(
    listener: TcpListener,
    storage_provider: Provider,
    config: GatewayConfig,
    cors_origin: Option<&Url>,
) -> Result<()>
where
    Storage: Store + 'static,
    Provider: StorageProvider<Storage> + Send + Sync + 'static,
{
    info!("Starting Noosphere gateway server...");

    let identity = config.expect_identity().await?;
    let owner_did = config.expect_owner_did().await?;

    debug!("This gateway's identity is {}", identity);
    debug!("This gateway is owned by {}", owner_did);

    let did_parser = DidParser::new(SUPPORTED_KEYS);

    let state = GatewayState::from_storage_provider(&storage_provider).await?;
    let block_store = Blocks::from_storage_provider(&storage_provider).await?;

    let mut cors = CorsLayer::new();

    if let Some(cors_origin) = cors_origin {
        cors = cors
            .allow_origin(
                cors_origin
                    .origin()
                    .unicode_serialization()
                    .as_str()
                    .parse::<HeaderValue>()?,
            )
            .allow_headers(Any)
            .allow_methods(vec![
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::PUT,
                Method::DELETE,
            ]);
    }

    let app = Router::new()
        .route("/api/v0alpha1/fetch", get(fetch_handler))
        .route("/api/v0alpha1/push", put(push_handler))
        .route("/api/v0alpha1/identify", get(identify_handler))
        .layer(cors)
        .layer(Extension(Arc::new(Mutex::new(state))))
        .layer(Extension(Arc::new(Mutex::new(block_store))))
        .layer(Extension(Arc::new(storage_provider)))
        .layer(Extension(Arc::new(config)))
        .layer(Extension(Arc::new(Mutex::new(did_parser))))
        .layer(TraceLayer::new_for_http());

    info!("Server binding to {:?}", listener);

    Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use noosphere::{
        authority::generate_ed25519_key,
        view::{Sphere, SphereMutation},
    };
    use noosphere_api::{
        client::Client,
        data::{PushBody, PushResponse},
    };

    use noosphere_storage::{interface::DagCborStore, memory::MemoryStore};
    use temp_dir::TempDir;
    use ucan::crypto::KeyMaterial;

    use crate::gateway::{
        commands::{initialize, serve},
        environment::{Blocks, GatewayRoot},
        tracing::initialize_tracing,
    };

    #[tokio::test]
    async fn it_can_be_identified_by_the_client_of_its_owner() {
        let owner_key_material = generate_ed25519_key();
        let owner_did = owner_key_material.get_did().await.unwrap();
        let root_dir = TempDir::new().unwrap();
        let root = GatewayRoot::at_path(&root_dir.path().to_path_buf());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let gateway_address = listener.local_addr().unwrap();

        let gateway_did = initialize(&root_dir.path().to_path_buf(), &owner_did)
            .await
            .unwrap();

        let config = root.to_config();
        let storage_provider = root.to_storage_provider().unwrap();

        let server_task = tokio::spawn(async move {
            serve(listener, storage_provider, config, None)
                .await
                .unwrap();
        });

        let client_task = tokio::spawn(async move {
            let uri = format!("http://{}:{}", gateway_address.ip(), gateway_address.port());
            let _client = Client::identify(&uri, &owner_key_material, None, Some(&gateway_did))
                .await
                .unwrap();

            server_task.abort();
            let _ = server_task.await;
        });

        client_task.await.unwrap();
    }

    #[tokio::test]
    async fn it_can_receive_a_newly_initialized_subspace_from_the_client() {
        //initialize_tracing();

        let owner_key_material = generate_ed25519_key();
        let owner_did = owner_key_material.get_did().await.unwrap();
        let root_dir = TempDir::new().unwrap();
        let root = GatewayRoot::at_path(&root_dir.path().to_path_buf());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let gateway_address = listener.local_addr().unwrap();

        let gateway_did = initialize(&root_dir.path().to_path_buf(), &owner_did)
            .await
            .unwrap();

        let config = root.to_config();
        let storage_provider = root.to_storage_provider().unwrap();

        let server_task = {
            let storage_provider = storage_provider.clone();
            tokio::spawn(async move {
                serve(listener, storage_provider, config, None)
                    .await
                    .unwrap();
            })
        };

        let client_task = tokio::spawn(async move {
            let mut memory_store = MemoryStore::default();
            let (sphere, sphere_proof, _) = Sphere::try_generate(&owner_did, &mut memory_store)
                .await
                .unwrap();

            let uri = format!("http://{}:{}", gateway_address.ip(), gateway_address.port());
            let client = Client::identify(
                &uri,
                &owner_key_material,
                Some(vec![sphere_proof]),
                Some(&gateway_did),
            )
            .await
            .unwrap();

            let sphere_did = sphere.try_get_identity().await.unwrap();
            let bundle = sphere.try_as_bundle().await.unwrap();

            let push_result = client
                .push(&PushBody {
                    sphere: sphere_did,
                    base: None,
                    tip: sphere.cid().clone(),
                    blocks: bundle.clone(),
                })
                .await
                .unwrap();

            server_task.abort();
            let _ = server_task.await;

            assert_eq!(push_result, PushResponse::Ok);

            let block_store = Blocks::from_storage_provider(&storage_provider)
                .await
                .unwrap();

            for cid in memory_store.get_stored_cids().await {
                debug!("Checking for {}", cid);
                assert!(&block_store.contains_cbor(&cid).await.unwrap());
            }
        });

        client_task.await.unwrap();
    }

    #[tokio::test]
    async fn it_can_update_an_existing_subspace_with_changes_from_the_client() {
        initialize_tracing();

        let owner_key_material = generate_ed25519_key();
        let owner_did = owner_key_material.get_did().await.unwrap();
        let root_dir = TempDir::new().unwrap();
        let root = GatewayRoot::at_path(&root_dir.path().to_path_buf());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let gateway_address = listener.local_addr().unwrap();

        let gateway_did = initialize(&root_dir.path().to_path_buf(), &owner_did)
            .await
            .unwrap();

        let config = root.to_config();
        let storage_provider = root.to_storage_provider().unwrap();

        let server_task = {
            let storage_provider = storage_provider.clone();
            tokio::spawn(async move {
                serve(listener, storage_provider, config, None)
                    .await
                    .unwrap();
            })
        };

        // TODO(#1): Move the "client" work to a task

        let client_task = tokio::spawn(async move {
            let mut memory_store = MemoryStore::default();
            let (sphere, sphere_proof, _) = Sphere::try_generate(&owner_did, &mut memory_store)
                .await
                .unwrap();

            let uri = format!("http://{}:{}", gateway_address.ip(), gateway_address.port());
            let client = Client::identify(
                &uri,
                &owner_key_material,
                Some(vec![sphere_proof.clone()]),
                Some(&gateway_did),
            )
            .await
            .unwrap();

            let sphere_did = sphere.try_get_identity().await.unwrap();
            let bundle = sphere.try_as_bundle().await.unwrap();
            let original_cid = sphere.cid().clone();

            let push_result = client
                .push(&PushBody {
                    sphere: sphere_did.clone(),
                    base: None,
                    tip: original_cid.clone(),
                    blocks: bundle.clone(),
                })
                .await
                .unwrap();

            debug!("WAK");
            assert_eq!(push_result, PushResponse::Ok);
            debug!("WAK2");

            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set("zero", sphere.cid());

            let mut revision = sphere.try_apply(&mutation).await.unwrap();
            let first_revision_cid = revision
                .try_sign(&owner_key_material, Some(&sphere_proof))
                .await
                .unwrap();

            let sphere = Sphere::at(&first_revision_cid, &memory_store);
            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set("one", &first_revision_cid);

            let mut revision = sphere.try_apply(&mutation).await.unwrap();
            let second_revision_cid = revision
                .try_sign(&owner_key_material, Some(&sphere_proof))
                .await
                .unwrap();

            let sphere = Sphere::at(&second_revision_cid, &memory_store);
            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set("two", &second_revision_cid);

            let mut revision = sphere.try_apply(&mutation).await.unwrap();
            let final_revision_cid = revision
                .try_sign(&owner_key_material, Some(&sphere_proof))
                .await
                .unwrap();

            debug!("BUNDLING");
            let next_bundle = Sphere::at(&final_revision_cid, &memory_store)
                .try_bundle_until_ancestor(Some(&first_revision_cid))
                .await
                .unwrap();

            debug!("SECOND PUSH");
            let push_result = client
                .push(&PushBody {
                    sphere: sphere_did,
                    base: Some(original_cid),
                    tip: final_revision_cid,
                    blocks: next_bundle.clone(),
                })
                .await
                .unwrap();

            debug!("SECOND PUSH DONE");
            server_task.abort();
            let _ = server_task.await;

            assert_eq!(push_result, PushResponse::Ok);

            let block_store = Blocks::from_storage_provider(&storage_provider)
                .await
                .unwrap()
                .into_store();

            memory_store.expect_replica_in(&block_store).await.unwrap();

            // for cid in memory_store.get_stored_cids().await {
            //     debug!("Checking for {}", cid);
            //     assert!(&block_store.contains_cbor(&cid).await.unwrap());
            // }
        });

        client_task.await.unwrap();
    }

    // #[tokio::test]
    // #[ignore = "TODO"]
    // async fn it_hydrates_revisions_synced_from_a_client() {}

    #[tokio::test]
    #[ignore = "TODO"]
    async fn it_can_serve_subspace_revisions_to_a_client() {}
}
