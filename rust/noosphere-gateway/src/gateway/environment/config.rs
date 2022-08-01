use async_once_cell::OnceCell;
use std::path::PathBuf;
use toml_edit::{table, value, Document, Item};

use anyhow::{anyhow, Result};
use async_std::fs::{read, write};

use crate::gateway::environment::GatewayRoot;

pub const GATEWAY_TABLE: &str = "gateway";
pub const IDENTITY_KEY: &str = "identity";
pub const OWNER_DID_KEY: &str = "owner_did";

pub const INSECURE_TABLE: &str = "insecure";
pub const PRIVATE_KEY_MNEMONIC_KEY: &str = "private_key_mnemonic";

pub struct GatewayConfig {
    path: PathBuf,
    toml: OnceCell<Document>,
}

impl GatewayConfig {
    pub fn from_root(root: &GatewayRoot) -> Self {
        GatewayConfig {
            path: root.config_toml().to_path_buf(),
            toml: OnceCell::new(),
        }
    }

    async fn read_config(&self) -> Result<Document> {
        debug!("Reading config at {:?}", self.path);
        let contents = String::from_utf8(read(&self.path).await.unwrap_or_default())?;
        debug!("CONFIG CONTENTS: {}", contents);
        Ok(contents.as_str().parse()?)
    }

    async fn try_get_toml(&self) -> Result<&Document> {
        self.toml
            .get_or_try_init(async {
                let mut toml = self.read_config().await.unwrap_or_default();

                if !toml.contains_table(GATEWAY_TABLE) {
                    toml[GATEWAY_TABLE] = table();
                }

                Ok(toml)
            })
            .await
    }

    async fn try_get_toml_mut(&mut self) -> Result<&mut Document> {
        self.try_get_toml().await?;
        let toml = self
            .toml
            .get_mut()
            .ok_or_else(|| anyhow!("Config didn't initialize!"))?;

        Ok(toml)
    }

    pub async fn get_raw_contents(&self) -> Result<String> {
        Ok(self.try_get_toml().await?.to_string())
    }

    pub async fn set_owner_did(&mut self, owner_did: &str) -> Result<()> {
        let path = self.path.clone();
        let toml = self.try_get_toml_mut().await?;
        toml[GATEWAY_TABLE][OWNER_DID_KEY] = value(owner_did);
        write(&path, toml.to_string()).await?;
        Ok(())
    }

    pub async fn set_identity(&mut self, identity: &str) -> Result<()> {
        let path = self.path.clone();
        let toml = self.try_get_toml_mut().await?;
        toml[GATEWAY_TABLE][IDENTITY_KEY] = value(identity);
        write(&path, toml.to_string()).await?;
        Ok(())
    }

    pub async fn set_insecure_mnemonic(&mut self, mnemonic: &str) -> Result<()> {
        let path = self.path.clone();
        let toml = self.try_get_toml_mut().await?;

        if !toml.contains_table(INSECURE_TABLE) {
            toml[INSECURE_TABLE] = table();
        }

        toml[INSECURE_TABLE][PRIVATE_KEY_MNEMONIC_KEY] = value(mnemonic);

        write(&path, toml.to_string()).await?;
        Ok(())
    }

    pub async fn get_owner_did(&self) -> Result<Option<String>> {
        Ok(self.try_get_toml().await?[GATEWAY_TABLE]
            .get(OWNER_DID_KEY)
            .map(|item| item.as_str().map(|str| str.to_string()))
            .unwrap_or(None))
    }

    pub async fn get_identity(&self) -> Result<Option<String>> {
        Ok(self.try_get_toml().await?[GATEWAY_TABLE]
            .get(IDENTITY_KEY)
            .map(|item| item.as_str().map(|str| str.to_string()))
            .unwrap_or(None))
    }

    pub async fn get_insecure_mnemonic(&self) -> Result<Option<String>> {
        let toml = self.try_get_toml().await?;
        Ok(match toml.get(INSECURE_TABLE) {
            Some(Item::Table(table)) => table
                .get(PRIVATE_KEY_MNEMONIC_KEY)
                .map(|item| item.as_str().map(|str| str.to_string()))
                .unwrap_or(None),
            _ => None,
        })
    }

    pub async fn get_hardware_enclave_mode(&self) -> Result<Option<String>> {
        // TODO(#6): Implement TPM support
        Ok(None)
    }

    pub async fn expect_owner_did(&self) -> Result<String> {
        self.get_owner_did()
            .await?
            .ok_or_else(|| anyhow!("No owner DID configured"))
    }

    pub async fn expect_identity(&self) -> Result<String> {
        self.get_identity()
            .await?
            .ok_or_else(|| anyhow!("No identity configured"))
    }
}
