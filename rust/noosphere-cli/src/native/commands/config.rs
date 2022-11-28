use anyhow::Result;
use noosphere::sphere::GATEWAY_URL;
use noosphere_core::data::Did;
use noosphere_storage::KeyValueStore;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;
use url::Url;

use crate::native::{workspace::Workspace, ConfigGetCommand, ConfigSetCommand};

pub const COUNTERPART: &str = "counterpart";
pub const DIFFTOOL: &str = "difftool";

#[derive(Serialize, Deserialize, Clone)]
pub struct ConfigContents {
    pub gateway_url: Option<Url>,
    pub counterpart: Option<Did>,
    pub difftool: Option<String>,
}

// TODO: Consider signing configuration values to head-off tampering
pub struct Config<'a> {
    workspace: &'a Workspace,
    contents: OnceCell<ConfigContents>,
}

impl<'a> From<&'a Workspace> for Config<'a> {
    fn from(workspace: &'a Workspace) -> Self {
        Config {
            workspace,
            contents: Default::default(),
        }
    }
}

impl<'a> Config<'a> {
    pub async fn read(&self) -> Result<&ConfigContents> {
        self.contents
            .get_or_try_init(|| async {
                let context = self.workspace.sphere_context().await?;
                let context = context.lock().await;

                let db = context.db();

                let gateway_url: Option<Url> = db.get_key(GATEWAY_URL).await?;
                let counterpart: Option<Did> = db.get_key(COUNTERPART).await?;
                let difftool: Option<String> = db.get_key(DIFFTOOL).await?;

                Ok(ConfigContents {
                    gateway_url,
                    counterpart,
                    difftool,
                })
            })
            .await
    }
}

pub async fn config_set(command: ConfigSetCommand, workspace: &Workspace) -> Result<()> {
    let context = workspace.sphere_context().await?;
    let context = context.lock().await;

    let mut db = context.db().clone();

    match command {
        ConfigSetCommand::GatewayUrl { url } => db.set_key(GATEWAY_URL, url).await?,
        ConfigSetCommand::Counterpart { did } => db.set_key(COUNTERPART, did).await?,
        ConfigSetCommand::Difftool { tool } => db.set_key(DIFFTOOL, tool).await?,
    };

    Ok(())
}

pub async fn config_get(command: ConfigGetCommand, workspace: &Workspace) -> Result<()> {
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
        ConfigGetCommand::Difftool => db.get_key::<_, String>(DIFFTOOL).await?,
    };

    if let Some(value) = value {
        println!("{value}");
    }

    Ok(())
}
