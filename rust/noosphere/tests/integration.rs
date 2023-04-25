#![cfg(test)]

use std::pin::Pin;

#[cfg(target_arch = "wasm32")]
use instant::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use noosphere_core::{data::ContentType, tracing::initialize_tracing};
use noosphere_sphere::{HasMutableSphereContext, SphereContentRead, SphereContentWrite};

use tokio::io::AsyncReadExt;
use tokio_stream::Stream;
use tokio_util::io::StreamReader;
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

#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::spawn;

#[cfg(target_arch = "wasm32")]
async fn spawn<F, O>(future: F) -> Result<O>
where
    F: std::future::Future<Output = O> + 'static,
{
    Ok(future.await)
}

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: Duration) {
    gloo_timers::future::sleep(duration).await
}

fn slow_content() -> Pin<Box<StreamReader<impl Stream<Item = Result<Bytes, std::io::Error>>, Bytes>>>
{
    Box::pin(StreamReader::new(try_stream! {
        for _ in 0..3 {
            sleep(Duration::from_millis(100)).await;
            yield Bytes::from(vec![0])
        }
    }))
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn single_player_single_device_end_to_end_workflow() {
    initialize_tracing(None);

    let (configuration, _temporary_directories) = platform_configuration();
    let key_name = "foobarbaz";

    // Create the sphere and write a file to it
    let sphere_identity = {
        let noosphere = NoosphereContext::new(configuration.clone()).unwrap();

        noosphere.create_key(key_name).await.unwrap();

        let SphereReceipt {
            identity: sphere_identity,
            ..
        } = noosphere.create_sphere(key_name).await.unwrap();

        let mut sphere_channel = noosphere
            .get_sphere_channel(&sphere_identity)
            .await
            .unwrap();

        sphere_channel
            .mutable()
            .write("foo", "text/plain", b"bar".as_ref(), None)
            .await
            .unwrap();

        sphere_channel.mutable().save(None).await.unwrap();

        sphere_identity
    };

    // Open the sphere later and read the file and write another file
    {
        let noosphere = NoosphereContext::new(configuration.clone()).unwrap();

        let mut sphere_channel = noosphere
            .get_sphere_channel(&sphere_identity)
            .await
            .unwrap();

        let mut file = sphere_channel
            .immutable()
            .read("foo")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(file.memo.content_type(), Some(ContentType::Text));

        let mut contents = String::new();
        file.contents.read_to_string(&mut contents).await.unwrap();

        assert_eq!(contents, "bar");

        sphere_channel
            .mutable()
            .write("cats", "text/subtext", b"are great".as_ref(), None)
            .await
            .unwrap();

        sphere_channel.mutable().save(None).await.unwrap();
    };
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn writes_do_not_block_reads() {
    let (configuration, _temporary_directories) = platform_configuration();
    let key_name = "foobar";

    let noosphere = NoosphereContext::new(configuration.clone()).unwrap();

    noosphere.create_key(key_name).await.unwrap();

    let SphereReceipt {
        identity: sphere_identity,
        ..
    } = noosphere.create_sphere(key_name).await.unwrap();

    let mut sphere_channel = noosphere
        .get_sphere_channel(&sphere_identity)
        .await
        .unwrap();

    sphere_channel
        .mutable()
        .write("cats", "text/subtext", b"are great".as_ref(), None)
        .await
        .unwrap();
    sphere_channel.mutable().save(None).await.unwrap();

    let write_task = spawn({
        let mut sphere_channel = sphere_channel.clone();
        async move {
            sphere_channel
                .mutable()
                .write("foo", "application/octet-stream", slow_content(), None)
                .await?;

            sphere_channel.mutable().save(None).await
        }
    });

    let mut file = sphere_channel
        .immutable()
        .read("cats")
        .await
        .unwrap()
        .unwrap();
    let mut content = String::new();
    file.contents.read_to_string(&mut content).await.unwrap();

    assert_eq!(content.as_str(), "are great");

    let pending_file = sphere_channel.immutable().read("foo").await.unwrap();

    assert!(pending_file.is_none());

    write_task.await.unwrap().unwrap();

    let pending_file = sphere_channel.immutable().read("foo").await.unwrap();

    assert!(pending_file.is_some());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn key_names_must_not_be_empty() {
    let (configuration, _temporary_directories) = platform_configuration();

    let noosphere = NoosphereContext::new(configuration.clone()).unwrap();

    assert!(noosphere.create_key("foo").await.is_ok());
    assert!(noosphere.create_key("").await.is_err());
}
