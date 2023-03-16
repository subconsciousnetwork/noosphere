use crate::cli::{CLICommand, CLIConfigFile};
use crate::utils;
use anyhow::{anyhow, Result};
use noosphere::key::InsecureKeyStorage;
use noosphere_ns::{DhtConfig, Multiaddr, BOOTSTRAP_PEERS};
use std::net::SocketAddr;
use ucan_key_support::ed25519::Ed25519KeyMaterial;
use url::Url;

/// Configuration for [NameSystemRunner], hydrated/resolved from CLI.
pub struct RunnerNodeConfig {
    pub key_material: Ed25519KeyMaterial,
    pub api_address: Option<SocketAddr>,
    pub listening_address: Option<Multiaddr>,
    pub peers: Vec<Multiaddr>,
    pub dht_config: DhtConfig,
    pub ipfs_api_url: Option<Url>,
}

impl RunnerNodeConfig {
    /// Create a [RunnerNodeConfig] from a [CLIConfigFile].
    async fn try_from_config(
        key_storage: &InsecureKeyStorage,
        config: CLIConfigFile,
    ) -> Result<Self> {
        let key_material = utils::get_key_material(key_storage, &config.key).await?;
        let dht_config = config.dht_config;
        let listening_address = config.listening_address;
        let api_address = config.api_address;
        let ipfs_api_url = config.ipfs_api_url;
        let mut peers = config.peers;
        if !config.no_default_peers {
            peers.extend_from_slice(&BOOTSTRAP_PEERS[..]);
        }

        Ok(RunnerNodeConfig {
            api_address,
            key_material,
            listening_address,
            peers,
            dht_config,
            ipfs_api_url,
        })
    }

    /// Create a [RunnerNodeConfig] from a [CLICommand].
    pub async fn try_from_command(
        command: CLICommand,
        key_storage: &InsecureKeyStorage,
    ) -> Result<RunnerNodeConfig> {
        match command {
            CLICommand::Run {
                config,
                key,
                peers,
                no_default_peers,
                listening_address,
                api_address,
                ipfs_api_url,
            } => match config {
                Some(config_path) => {
                    let toml_str = tokio::fs::read_to_string(&config_path).await?;
                    let config: CLIConfigFile = toml::from_str(&toml_str)?;
                    Ok(RunnerNodeConfig::try_from_config(key_storage, config).await?)
                }
                None => {
                    let key_name: String =
                        key.ok_or_else(|| anyhow!("--key or --config must be provided."))?;

                    let bootstrap_peers = if let Some(peers) = peers {
                        peers
                    } else {
                        vec![]
                    };

                    let dht_config = DhtConfig::default();

                    let config = CLIConfigFile {
                        key: key_name.clone(),
                        listening_address,
                        api_address,
                        peers: bootstrap_peers,
                        no_default_peers,
                        dht_config,
                        ipfs_api_url,
                    };
                    Ok(RunnerNodeConfig::try_from_config(key_storage, config).await?)
                }
            },
            _ => Err(anyhow!(
                "Only CLICommand::Run can be converted into a Runner."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere::key::KeyStorage;
    use std::path::PathBuf;
    use tempdir::TempDir;
    use ucan::crypto::KeyMaterial;

    async fn keys_equal(key_1: &Ed25519KeyMaterial, key_2: &Ed25519KeyMaterial) -> Result<bool> {
        Ok(key_1.get_did().await? == key_2.get_did().await?)
    }

    struct Env {
        _dir: TempDir,
        pub dir_path: PathBuf,
        pub config_path: Option<PathBuf>,
        pub key_storage: InsecureKeyStorage,
    }

    impl Env {
        pub fn new() -> Result<Self> {
            let dir = TempDir::new("noosphere")?;
            let dir_path = dir.path().to_owned();
            let key_storage = InsecureKeyStorage::new(&dir_path)?;

            Ok(Env {
                _dir: dir,
                dir_path,
                key_storage,
                config_path: None,
            })
        }

        pub async fn new_with_config(config_str: &str) -> Result<Self> {
            let mut env = Env::new()?;
            let config_path = env.dir_path.join("config.toml");
            tokio::fs::write(&config_path, config_str.as_bytes()).await?;
            env.config_path = Some(config_path);
            Ok(env)
        }

        pub async fn create_key(&self, key_name: &str) -> Result<Ed25519KeyMaterial> {
            self.key_storage.create_key(key_name).await
        }
    }

    #[tokio::test]
    async fn try_from_command() -> Result<()> {
        let env = Env::new()?;
        let expected_key = env.create_key("single-test-key").await?;

        let config = RunnerNodeConfig::try_from_command(
            CLICommand::Run {
                api_address: None,
                config: None,
                key: Some(String::from("single-test-key")),
                listening_address: Some("/ip4/127.0.0.1/tcp/6666".parse()?),
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
            &env.key_storage,
        )
        .await?;

        assert!(
            keys_equal(&config.key_material, &expected_key).await?,
            "expected key material"
        );
        assert_eq!(
            config.listening_address.as_ref().unwrap(),
            &"/ip4/127.0.0.1/tcp/6666".parse()?,
            "expected listening_address"
        );
        assert_eq!(config.peers.len(), 1, "expected default bootstrap peers");
        assert_eq!(
            config.peers.get(0),
            BOOTSTRAP_PEERS[..].get(0),
            "expected default bootstrap peers"
        );

        Ok(())
    }

    #[tokio::test]
    async fn try_from_command_with_config() -> Result<()> {
        let env = Env::new_with_config(
            r#"
key = "my-bootstrap-key-1"
listening_address = 10000
peers = [
    "/ip4/127.0.0.1/tcp/10001"
]
"#,
        )
        .await?;

        let key_1 = env.create_key("my-bootstrap-key-1").await?;

        let config = RunnerNodeConfig::try_from_command(
            CLICommand::Run {
                api_address: None,
                config: env.config_path.to_owned(),
                key: None,
                listening_address: None,
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
            &env.key_storage,
        )
        .await?;

        assert!(
            keys_equal(&config.key_material, &key_1).await?,
            "expected key material"
        );
        assert_eq!(
            config.listening_address.as_ref().unwrap(),
            &"/ip4/127.0.0.1/tcp/10000".parse()?,
            "expected listening_address"
        );
        assert!(
            config
                .peers
                .contains(&("/ip4/127.0.0.1/tcp/10001".to_string().parse()?)),
            "expected explicit peer"
        );
        assert!(
            config.peers.contains(BOOTSTRAP_PEERS[..].get(0).unwrap()),
            "expected default peer"
        );
        assert_eq!(config.peers.len(), 2, "expected 2 peers");
        Ok(())
    }

    #[tokio::test]
    async fn try_from_command_with_config_no_default_peers() -> Result<()> {
        let env = Env::new_with_config(
            r#"
key = "my-bootstrap-key-1"
listening_address = 10000
peers = [
    "/ip4/127.0.0.1/tcp/10001"
]
no_default_peers = true
"#,
        )
        .await?;

        let _ = env.create_key("my-bootstrap-key-1").await?;
        let config = RunnerNodeConfig::try_from_command(
            CLICommand::Run {
                api_address: None,
                config: env.config_path.to_owned(),
                key: None,
                listening_address: None,
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
            &env.key_storage,
        )
        .await?;
        assert!(
            config
                .peers
                .contains(&("/ip4/127.0.0.1/tcp/10001".to_string().parse()?)),
            "expected explicit peer"
        );
        assert_eq!(config.peers.len(), 1, "expected 1 peer");
        Ok(())
    }

    #[tokio::test]
    async fn try_from_command_validation() -> Result<()> {
        let env = Env::new_with_config(
            r#"
key = "my-bootstrap-key-1"
listening_address = 10000
"#,
        )
        .await?;

        let commands = [
            CLICommand::Run {
                api_address: None,
                config: None,
                key: None,
                listening_address: Some("/ip4/127.0.0.1/tcp/6666".parse()?),
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
            CLICommand::Run {
                api_address: None,
                config: None,
                key: Some(String::from("key-does-not-exist")),
                listening_address: Some("/ip4/127.0.0.1/tcp/6666".parse()?),
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
            CLICommand::Run {
                api_address: None,
                config: env.config_path.to_owned(),
                key: None,
                listening_address: None,
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
            CLICommand::Run {
                api_address: None,
                config: Some(env.dir_path.join("invalid_path")),
                key: None,
                listening_address: None,
                peers: None,
                no_default_peers: false,
                ipfs_api_url: None,
            },
        ];

        for command in commands {
            assert!(
                RunnerNodeConfig::try_from_command(command, &env.key_storage)
                    .await
                    .is_err(),
                "expected failure from try_from_command"
            );
        }

        Ok(())
    }
}
