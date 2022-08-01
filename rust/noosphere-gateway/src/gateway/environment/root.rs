use anyhow::Result;
use std::path::{Path, PathBuf};

use noosphere_storage::native::{NativeStorageInit, NativeStorageProvider};

use super::GatewayConfig;

pub const NOOSPHERE_STORE_NAME: &str = "noosphere.store";
pub const CONFIG_TOML_NAME: &str = "config.toml";

pub struct GatewayRoot {
    path: PathBuf,
    noosphere_store: PathBuf,
    config_toml: PathBuf,
}

impl GatewayRoot {
    pub fn at_path(path: &PathBuf) -> Self {
        let root = path.as_path();
        let noosphere_store = root.join(NOOSPHERE_STORE_NAME);
        let config_toml = root.join(CONFIG_TOML_NAME);

        GatewayRoot {
            path: path.clone(),
            noosphere_store,
            config_toml,
        }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn noosphere_store(&self) -> &Path {
        self.noosphere_store.as_path()
    }

    pub fn config_toml(&self) -> &Path {
        self.config_toml.as_path()
    }

    pub fn to_storage_provider(&self) -> Result<NativeStorageProvider> {
        NativeStorageProvider::new(NativeStorageInit::Path(self.noosphere_store().into()))
    }

    pub fn to_config(&self) -> GatewayConfig {
        GatewayConfig::from_root(self)
    }
}
