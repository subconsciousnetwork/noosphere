use anyhow::Result;
use noosphere_core::context::metadata::{COUNTERPART, GATEWAY_URL};
use noosphere_core::data::Did;
use noosphere_storage::KeyValueStore;

use url::Url;

use crate::native::{cli::ConfigGetCommand, cli::ConfigSetCommand, workspace::Workspace};

/// Set a local metadata value in the database
pub async fn config_set(command: ConfigSetCommand, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;
    let context = workspace.sphere_context().await?;
    let context = context.lock().await;

    let mut db = context.db().clone();

    match command {
        ConfigSetCommand::GatewayUrl { url } => db.set_key(GATEWAY_URL, url).await?,
        ConfigSetCommand::Counterpart { did } => db.set_key(COUNTERPART, did).await?,
    };

    Ok(())
}

/// Get a local metadata value from the database
pub async fn config_get(command: ConfigGetCommand, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;
    let context = workspace.sphere_context().await?;
    let context = context.lock().await;

    let db = context.db();

    let value = match command {
        ConfigGetCommand::GatewayUrl => db
            .get_key::<_, Url>(GATEWAY_URL)
            .await?
            .map(|url| url.to_string()),
        ConfigGetCommand::Counterpart => db
            .get_key::<_, Did>(COUNTERPART)
            .await?
            .map(|did| did.to_string()),
    };

    if let Some(value) = value {
        info!("{value}");
    }

    Ok(())
}
