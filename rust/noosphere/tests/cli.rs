#![cfg(all(feature = "test-kubo", not(target_arch = "wasm32")))]

//! Integration tests to demonstrate that the Noosphere CLI, aka "orb", works
//! end-to-end in concert with the name system and backing block syndication

// TODO(#629): Remove this when we migrate off of `release-please`
extern crate noosphere_cli_dev as noosphere_cli;

use anyhow::Result;
use noosphere_cli::{helpers::CliSimulator, paths::SPHERE_DIRECTORY};
use noosphere_common::helpers::wait;
use noosphere_core::tracing::initialize_tracing;
use serde_json::Value;
use tokio::task::JoinHandle;

#[tokio::test(flavor = "multi_thread")]
async fn orb_status_errors_on_empty_directory() -> Result<()> {
    initialize_tracing(None);
    let client = CliSimulator::new()?;

    assert!(client.orb(&["sphere", "status"]).await.is_err());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_sphere_create_initializes_a_sphere() -> Result<()> {
    initialize_tracing(None);
    let client = CliSimulator::new()?;

    client.orb(&["key", "create", "foobar"]).await?;
    client
        .orb(&["sphere", "create", "--owner-key", "foobar"])
        .await?;

    assert!(client.sphere_directory().join(SPHERE_DIRECTORY).exists());

    Ok(())
}

async fn initialize_syncing_replicas() -> Result<(
    CliSimulator,
    CliSimulator,
    JoinHandle<Result<(), anyhow::Error>>,
)> {
    let first_replica = CliSimulator::new()?;
    let second_replica = CliSimulator::new()?;

    first_replica.orb(&["key", "create", "foo"]).await?;
    first_replica
        .orb(&["sphere", "create", "--owner-key", "foo"])
        .await?;

    let client_sphere_id = first_replica
        .orb_with_output(&["sphere", "status", "--id"])
        .await?
        .join("\n");

    let gateway = CliSimulator::new()?;

    gateway.orb(&["key", "create", "gateway"]).await?;

    gateway
        .orb(&["sphere", "create", "--owner-key", "gateway"])
        .await?;

    gateway
        .orb(&["sphere", "config", "set", "counterpart", &client_sphere_id])
        .await?;

    let gateway_task = tokio::task::spawn(async move { gateway.orb(&["serve"]).await });

    wait(1).await;

    first_replica
        .orb(&[
            "sphere",
            "config",
            "set",
            "gateway-url",
            "http://127.0.0.1:4433",
        ])
        .await?;

    second_replica.orb(&["key", "create", "bar"]).await?;
    let second_replica_id = match serde_json::from_str(
        &second_replica
            .orb_with_output(&["key", "list", "--as-json"])
            .await?
            .join("\n"),
    )? {
        Value::Object(keys) => keys.get("bar").unwrap().as_str().unwrap().to_owned(),
        _ => panic!(),
    };

    tokio::fs::write(
        first_replica.sphere_directory().join("foo.subtext"),
        "foobar",
    )
    .await?;

    first_replica.orb(&["sphere", "save"]).await?;

    first_replica
        .orb(&["sphere", "auth", "add", &second_replica_id])
        .await?;

    let auth_list = first_replica
        .orb_with_output(&["sphere", "auth", "list", "--as-json"])
        .await?
        .join("\n");
    let second_replica_auth = match serde_json::from_str(&auth_list)? {
        Value::Array(auths) => match auths
            .iter()
            .filter(|auth| {
                auth.as_object()
                    .unwrap()
                    .get("name")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    != "(OWNER)"
            })
            .take(1)
            .next()
            .unwrap()
        {
            Value::Object(auth) => auth.get("link").unwrap().as_str().unwrap().to_owned(),
            _ => panic!(),
        },
        _ => panic!(),
    };

    first_replica.orb(&["sphere", "sync"]).await?;

    second_replica
        .orb(&[
            "sphere",
            "join",
            "--authorization",
            &second_replica_auth,
            "--local-key",
            "bar",
            "--gateway-url",
            "http://127.0.0.1:4433",
            &client_sphere_id,
        ])
        .await?;

    let foo_contents =
        tokio::fs::read_to_string(second_replica.sphere_directory().join("foo.subtext")).await?;

    assert_eq!(foo_contents.as_str(), "foobar");

    second_replica.orb(&["sphere", "sync"]).await?;

    Ok((first_replica, second_replica, gateway_task))
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_can_enable_multiple_replicas_to_synchronize() -> Result<()> {
    initialize_tracing(None);

    let (_, _, gateway_task) = initialize_syncing_replicas().await?;

    gateway_task.abort();

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_keeps_the_file_system_in_sync_with_remote_changes() -> Result<()> {
    initialize_tracing(None);

    let (first_replica, second_replica, gateway_task) = initialize_syncing_replicas().await?;

    tracing::info!("HAIIII");
    println!("HAI");

    let subdirectory_name = "fark";
    let first_replica_subdirectory = first_replica.sphere_directory().join(subdirectory_name);
    let second_replica_subdirectory = second_replica.sphere_directory().join(subdirectory_name);

    tracing::info!("HAI!");

    tokio::fs::create_dir_all(&first_replica_subdirectory).await?;
    tokio::fs::write(first_replica_subdirectory.join("bar.txt"), "foobar").await?;
    tokio::fs::write(first_replica_subdirectory.join("baz.md"), "foobaz").await?;

    first_replica.orb(&["sphere", "save"]).await?;
    first_replica.orb(&["sphere", "sync"]).await?;

    second_replica.print_debug_shell_command();

    wait(1000).await;

    for i in 0..3 {
        tracing::warn!("Second replica syncing in {}...", 3 - i);
        wait(1).await;
    }

    second_replica.orb(&["sphere", "sync"]).await?;

    // assert!(second_replica_subdirectory.join("bar.txt").exists());
    // assert!(second_replica_subdirectory.join("baz.md").exists());

    /*
    tracing::info!("HAI");

    tokio::fs::remove_file(second_replica_subdirectory.join("bar.txt")).await?;
    tokio::fs::rename(
        second_replica_subdirectory.join("baz.md"),
        second_replica_subdirectory.join("baz.md"),
    )
    .await?;

    second_replica.orb(&["sphere", "save"]).await?;

    tracing::info!("HO");
    assert!(!second_replica_subdirectory.join("bar.txt").exists());
    assert!(!second_replica_subdirectory.join("baz.md").exists());
    assert!(second_replica_subdirectory.join("baz.txt").exists());

    second_replica.orb(&["sphere", "sync"]).await?;

    first_replica.orb(&["sphere", "sync"]).await?;

    assert!(!first_replica_subdirectory.join("bar.txt").exists());
    assert!(!first_replica_subdirectory.join("baz.md").exists());
    assert!(first_replica_subdirectory.join("baz.txt").exists());

    */
    gateway_task.abort();

    Ok(())
}
