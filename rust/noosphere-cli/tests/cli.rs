#![cfg(not(target_arch = "wasm32"))]

#[macro_use]
extern crate tracing;

mod helpers;

use anyhow::Result;
use helpers::CliTestEnvironment;
use noosphere_cli::native::paths::SPHERE_DIRECTORY;
use noosphere_core::tracing::initialize_tracing;
use serde_json::Value;

use crate::helpers::wait;

#[tokio::test(flavor = "multi_thread")]
async fn orb_status_errors_on_empty_directory() -> Result<()> {
    initialize_tracing(None);
    let client = CliTestEnvironment::new()?;

    assert!(client.orb(&["status"]).await.is_err());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_sphere_create_initializes_a_sphere() -> Result<()> {
    initialize_tracing(None);
    let client = CliTestEnvironment::new()?;

    client.orb(&["key", "create", "foobar"]).await?;
    client
        .orb(&["sphere", "create", "--owner-key", "foobar"])
        .await?;

    assert!(client.sphere_directory().join(SPHERE_DIRECTORY).exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_can_enable_multiple_replicas_to_synchronize() -> Result<()> {
    initialize_tracing(None);

    let first_replica = CliTestEnvironment::new()?;
    let second_replica = CliTestEnvironment::new()?;

    first_replica.orb(&["key", "create", "foo"]).await?;
    first_replica
        .orb(&["sphere", "create", "--owner-key", "foo"])
        .await?;

    let client_sphere_id = first_replica
        .orb_with_output(&["status", "--id"])
        .await?
        .join("\n");

    let gateway = CliTestEnvironment::new()?;

    gateway.orb(&["key", "create", "gateway"]).await?;

    gateway
        .orb(&["sphere", "create", "--owner-key", "gateway"])
        .await?;

    gateway
        .orb(&["config", "set", "counterpart", &client_sphere_id])
        .await?;

    let gateway_task = tokio::task::spawn(async move { gateway.orb(&["serve"]).await });

    wait(1).await;

    first_replica
        .orb(&["config", "set", "gateway-url", "http://127.0.0.1:4433"])
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

    first_replica.orb(&["save"]).await?;

    first_replica
        .orb(&["auth", "add", &second_replica_id])
        .await?;

    let second_replica_auth = match serde_json::from_str(
        &first_replica
            .orb_with_output(&["auth", "list", "--as-json"])
            .await?
            .join("\n"),
    )? {
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

    first_replica.orb(&["sync"]).await?;

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

    gateway_task.abort();

    Ok(())
}
