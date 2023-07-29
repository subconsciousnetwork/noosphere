pub mod cli;
pub mod commands;
pub mod content;
pub mod extension;
pub mod paths;
pub mod render;
pub mod workspace;

use anyhow::Result;

use noosphere_core::tracing::initialize_tracing;

use clap::Parser;

use self::{
    cli::{AuthCommand, Cli, ConfigCommand, KeyCommand, OrbCommand, SphereCommand},
    commands::{
        auth::{auth_add, auth_list, auth_revoke},
        config::{config_get, config_set},
        key::{key_create, key_list},
        save::save,
        serve::serve,
        sphere::{sphere_create, sphere_follow, sphere_join, sphere_unfollow},
        status::status,
        sync::sync,
    },
    workspace::Workspace,
};

pub async fn main() -> Result<()> {
    initialize_tracing(None);

    let workspace = Workspace::new(&std::env::current_dir()?, None)?;

    invoke_cli(Cli::parse(), workspace).await
}

pub async fn invoke_cli(cli: Cli, mut workspace: Workspace) -> Result<()> {
    match cli.command {
        OrbCommand::Key { command } => match command {
            KeyCommand::Create { name } => key_create(&name, &workspace).await?,
            KeyCommand::List { as_json } => key_list(as_json, &workspace).await?,
        },
        OrbCommand::Sphere { command } => match command {
            SphereCommand::Create { owner_key } => {
                sphere_create(&owner_key, &mut workspace).await?;
            }
            SphereCommand::Join {
                local_key,
                authorization,
                id,
                gateway_url,
            } => {
                sphere_join(&local_key, authorization, &id, &gateway_url, &mut workspace).await?;
            }
            SphereCommand::Auth { command } => match command {
                AuthCommand::Add { did, name } => {
                    auth_add(&did, name, &workspace).await?;
                }
                AuthCommand::List { tree, as_json } => auth_list(tree, as_json, &workspace).await?,
                AuthCommand::Revoke { name } => auth_revoke(&name, &workspace).await?,
                AuthCommand::Rotate {} => todo!(),
            },
            SphereCommand::Config { command } => match command {
                ConfigCommand::Set { command } => config_set(command, &workspace).await?,
                ConfigCommand::Get { command } => config_get(command, &workspace).await?,
            },

            SphereCommand::Status { id } => status(id, &workspace).await?,
            SphereCommand::Save => save(&workspace).await?,
            SphereCommand::Sync { auto_retry } => sync(auto_retry, &workspace).await?,
            SphereCommand::Follow { name, did } => {
                sphere_follow(name, did, &workspace).await?;
            }
            SphereCommand::Unfollow { name } => {
                sphere_unfollow(name, &workspace).await?;
            }
        },

        OrbCommand::Serve {
            cors_origin,
            ipfs_api,
            name_resolver_api,
            interface,
            port,
        } => {
            serve(
                interface,
                port,
                ipfs_api,
                name_resolver_api,
                cors_origin,
                &workspace,
            )
            .await?
        }
    };

    Ok(())
}
