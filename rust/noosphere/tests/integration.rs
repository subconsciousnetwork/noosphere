#![cfg(test)]

#[macro_use]
extern crate tracing;

use noosphere_core::{data::ContentType, tracing::initialize_tracing};
use noosphere_sphere::{
    HasMutableSphereContext, SphereContentRead, SphereContentWrite, SphereCursor,
};

use tokio::io::AsyncReadExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use noosphere::{
    sphere::SphereReceipt, NoosphereContext, NoosphereContextConfiguration, NoosphereNetwork,
    NoosphereSecurity, NoosphereStorage,
};

#[cfg(target_arch = "wasm32")]
fn platform_configuration() -> (NoosphereContextConfiguration, ()) {
    let configuration = NoosphereContextConfiguration {
        storage: NoosphereStorage::Scoped {
            path: "sphere-data".into(),
        },
        security: NoosphereSecurity::Opaque,
        network: NoosphereNetwork::Http {
            gateway_api: None,
            ipfs_gateway_url: None,
        },
    };

    (configuration, ())
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_configuration() -> (
    NoosphereContextConfiguration,
    (tempfile::TempDir, tempfile::TempDir),
) {
    use tempfile::TempDir;

    let global_storage = TempDir::new().unwrap();
    let sphere_storage = TempDir::new().unwrap();

    let configuration = NoosphereContextConfiguration {
        storage: NoosphereStorage::Unscoped {
            path: sphere_storage.path().to_path_buf(),
        },
        security: NoosphereSecurity::Insecure {
            path: global_storage.path().to_path_buf(),
        },
        network: NoosphereNetwork::Http {
            gateway_api: None,
            ipfs_gateway_url: None,
        },
    };

    (configuration, (global_storage, sphere_storage))
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn single_player_single_device_end_to_end_workflow() {
    initialize_tracing();

    info!("A");
    let (configuration, _temporary_directories) = platform_configuration();
    let key_name = "foobar";

    // Create the sphere and write a file to it
    let sphere_identity = {
        let noosphere = NoosphereContext::new(configuration.clone()).unwrap();

        noosphere.create_key(key_name).await.unwrap();

        let SphereReceipt {
            identity: sphere_identity,
            ..
        } = noosphere.create_sphere(key_name).await.unwrap();

        let sphere_context = noosphere
            .get_sphere_context(&sphere_identity)
            .await
            .unwrap();

        info!("A.A");
        let mut sphere_cursor = SphereCursor::latest(sphere_context);

        info!("A.B");
        sphere_cursor
            .write("foo", "text/plain", b"bar".as_ref(), None)
            .await
            .unwrap();

        info!("A.C");
        sphere_cursor.save(None).await.unwrap();

        info!("A.D");
        sphere_identity
    };

    info!("B");
    // Open the sphere later and read the file and write another file
    {
        let noosphere = NoosphereContext::new(configuration.clone()).unwrap();

        let sphere_context = noosphere
            .get_sphere_context(&sphere_identity)
            .await
            .unwrap();

        let mut sphere_cursor = SphereCursor::latest(sphere_context);

        let mut file = sphere_cursor.read("foo").await.unwrap().unwrap();

        assert_eq!(
            file.memo.content_type(),
            Some(ContentType::Unknown("text/plain".into()))
        );

        let mut contents = String::new();
        file.contents.read_to_string(&mut contents).await.unwrap();

        assert_eq!(contents, "bar");

        sphere_cursor
            .write("cats", "text/subtext", b"are great".as_ref(), None)
            .await
            .unwrap();

        sphere_cursor.save(None).await.unwrap();
    };

    info!("C");
}
