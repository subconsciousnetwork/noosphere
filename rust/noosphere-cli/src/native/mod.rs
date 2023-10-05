//! Functions for invoking the CLI imperatively

pub mod cli;
pub mod commands;
pub mod content;
pub mod extension;
pub mod paths;
pub mod render;
pub mod workspace;

#[cfg(any(test, feature = "helpers"))]
pub mod helpers;

use std::path::{Path, PathBuf};

use self::{
    cli::{AuthCommand, Cli, ConfigCommand, FollowCommand, KeyCommand, OrbCommand, SphereCommand},
    commands::{
        key::{key_create, key_list},
        serve::serve,
        sphere::{
            auth_add, auth_list, auth_revoke, config_get, config_set, follow_add, follow_list,
            follow_remove, follow_rename, history, save, sphere_create, sphere_join, status, sync,
        },
    },
    workspace::Workspace,
};
use anyhow::Result;
use clap::Parser;
use noosphere_core::tracing::initialize_tracing;
use noosphere_storage::StorageConfig;

/// Additional context used to invoke a [Cli] command.
pub struct CliContext<'a> {
    /// Path to the current working directory.
    cwd: PathBuf,
    /// Path to the global configuration directory, if provided.
    global_config_directory: Option<&'a Path>,
}

#[cfg(not(doc))]
#[allow(missing_docs)]
pub async fn main() -> Result<()> {
    initialize_tracing(None);
    let context = CliContext {
        cwd: std::env::current_dir()?,
        global_config_directory: None,
    };
    invoke_cli(Cli::parse(), &context).await
}

/// Invoke the CLI implementation imperatively.
///
/// This is the entrypoint used by orb when handling a command line invocation.
/// The [Cli] is produced by parsing the command line arguments, and internally
/// creates a new [Workspace] from the current working directory.
///
/// Use [invoke_cli_with_workspace] if using your own [Workspace].
pub async fn invoke_cli<'a>(cli: Cli, context: &CliContext<'a>) -> Result<()> {
    let storage_config = if let OrbCommand::Serve {
        storage_memory_cache_limit,
        ..
    } = &cli.command
    {
        Some(StorageConfig {
            memory_cache_limit: *storage_memory_cache_limit,
        })
    } else {
        None
    };
    let workspace = Workspace::new(
        &context.cwd,
        context.global_config_directory,
        storage_config,
    )?;

    invoke_cli_with_workspace(cli, workspace).await
}

/// Same as [invoke_cli], but enables the caller to provide their own
/// initialized [Workspace]
pub async fn invoke_cli_with_workspace(cli: Cli, mut workspace: Workspace) -> Result<()> {
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
                render_depth,
            } => {
                sphere_join(
                    &local_key,
                    authorization,
                    &id,
                    &gateway_url,
                    render_depth,
                    &mut workspace,
                )
                .await?;
            }
            SphereCommand::Auth { command } => match command {
                AuthCommand::Add { did, name } => {
                    auth_add(&did, name, &workspace).await?;
                }
                AuthCommand::List { tree, as_json } => auth_list(tree, as_json, &workspace).await?,
                AuthCommand::Revoke { name } => auth_revoke(&name, &workspace).await?,
                AuthCommand::Rotate {} => unimplemented!(),
            },
            SphereCommand::Config { command } => match command {
                ConfigCommand::Set { command } => config_set(command, &workspace).await?,
                ConfigCommand::Get { command } => config_get(command, &workspace).await?,
            },

            SphereCommand::Status { id } => status(id, &workspace).await?,
            SphereCommand::Save { render_depth } => save(render_depth, &workspace).await?,
            SphereCommand::Sync {
                auto_retry,
                render_depth,
            } => sync(auto_retry, render_depth, &workspace).await?,
            SphereCommand::Follow { command } => match command {
                FollowCommand::Add { name, sphere_id } => {
                    follow_add(name, sphere_id, &workspace).await?;
                }
                FollowCommand::Remove {
                    by_name,
                    by_sphere_id,
                } => follow_remove(by_name, by_sphere_id, &workspace).await?,
                FollowCommand::Rename { from, to } => follow_rename(from, to, &workspace).await?,
                FollowCommand::List { as_json } => follow_list(as_json, &workspace).await?,
            },
            SphereCommand::Render { render_depth } => {
                commands::sphere::render(render_depth, &workspace).await?
            }
            SphereCommand::History => {
                history(&workspace).await?;
            }
        },

        OrbCommand::Serve {
            cors_origin,
            ipfs_api,
            iroh_ticket,
            name_resolver_api,
            interface,
            port,
            ..
        } => {
            serve(
                interface,
                port,
                ipfs_api,
                iroh_ticket,
                name_resolver_api,
                cors_origin,
                &mut workspace,
            )
            .await?;
        }
    }

    Ok(())
}
