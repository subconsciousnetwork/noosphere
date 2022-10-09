use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::OnceCell;
use url::Url;

use crate::native::{
    workspace::{self, Workspace},
    ConfigGetCommand, ConfigSetCommand,
};

#[derive(Serialize, Deserialize, Clone)]
pub struct ConfigContents {
    pub gateway_url: Option<Url>,
    pub counterpart: Option<String>,
    pub difftool: Option<String>,
}

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
            .get_or_try_init(|| {
                let config_path = self.workspace.config_path().clone();
                async {
                    toml_edit::de::from_str(&fs::read_to_string(config_path).await?)
                        .map_err(|error| anyhow!(error))
                }
            })
            .await
    }

    pub async fn write(&mut self, contents: &ConfigContents) -> Result<()> {
        fs::write(
            self.workspace.config_path(),
            toml_edit::ser::to_vec(contents)?,
        )
        .await?;
        self.contents = OnceCell::new();
        Ok(())
    }
}

pub async fn config_set(command: ConfigSetCommand, workspace: &Workspace) -> Result<()> {
    workspace.expect_local_directories()?;

    let mut config = Config::from(workspace);
    let mut config_contents = config.read().await?.clone();

    match command {
        ConfigSetCommand::GatewayUrl { url } => config_contents.gateway_url = Some(url),
        ConfigSetCommand::Counterpart { did } => config_contents.counterpart = Some(did),
        ConfigSetCommand::Difftool { tool } => config_contents.difftool = Some(tool),
    };

    config.write(&config_contents).await?;

    Ok(())
}

pub async fn config_get(command: ConfigGetCommand, workspace: &Workspace) -> Result<()> {
    workspace.expect_local_directories()?;

    let config = Config::from(workspace);
    let config_contents = config.read().await?.clone();

    let value = match command {
        ConfigGetCommand::GatewayUrl => config_contents.gateway_url.map(|url| url.to_string()),
        ConfigGetCommand::Counterpart => config_contents.counterpart,
        ConfigGetCommand::Difftool => config_contents.difftool,
    };

    if let Some(value) = value {
        println!("{value}");
    }

    Ok(())
}
