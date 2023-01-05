use crate::cli::{CLICommand, CLIConfigFile, CLIConfigFileNode};
use crate::utils;
use anyhow::{anyhow, Result};
use noosphere::key::InsecureKeyStorage;
use noosphere_ns::{DHTConfig, Multiaddr, BOOTSTRAP_PEERS};

use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// Configuration for a name system node configuration containing
/// resolved data.
pub struct RunnerNodeConfig {
    pub key_material: Ed25519KeyMaterial,
    pub listening_address: Multiaddr,
    pub peers: Vec<Multiaddr>,
    pub dht_config: DHTConfig,
}

/// Configuration for [Runner], hydrated/resolved from CLI.
pub struct RunnerConfig {
    pub nodes: Vec<RunnerNodeConfig>,
}

impl RunnerConfig {
    fn new(nodes: Vec<RunnerNodeConfig>) -> Self {
        RunnerConfig { nodes }
    }

    async fn try_from_config(
        key_storage: &InsecureKeyStorage,
        config: CLIConfigFile,
    ) -> Result<Self> {
        let mut nodes: Vec<RunnerNodeConfig> = vec![];
        for config_node in config.nodes {
            let dht_config = config_node.dht_config;
            let port = config_node.port.unwrap_or(0);
            let key_material = utils::get_key_material(key_storage, &config_node.key).await?;
            let listening_address = utils::create_listening_address(port);
            let mut peers = config_node.peers;
            if let Some(ref global_peers) = config.peers {
                peers.append(&mut global_peers.clone());
            }
            peers.sort();
            peers.dedup();

            nodes.push(RunnerNodeConfig {
                key_material,
                listening_address,
                peers,
                dht_config,
            })
        }
        Ok(RunnerConfig::new(nodes))
    }

    pub async fn try_from_command(
        key_storage: &InsecureKeyStorage,
        command: CLICommand,
    ) -> Result<RunnerConfig> {
        match command {
            CLICommand::Run {
                config,
                key,
                mut bootstrap,
                mut port,
            } => match config {
                Some(config_path) => {
                    let toml_str = tokio::fs::read_to_string(&config_path).await?;
                    let config: CLIConfigFile = toml::from_str(&toml_str)?;
                    Ok(RunnerConfig::try_from_config(key_storage, config).await?)
                }
                None => {
                    let key_name: String =
                        key.ok_or_else(|| anyhow!("--key or --config must be provided."))?;

                    let peers = if let Some(bootstrap_peers) = bootstrap.take() {
                        bootstrap_peers
                    } else {
                        BOOTSTRAP_PEERS[..].to_vec()
                    };

                    let config = CLIConfigFile {
                        nodes: vec![CLIConfigFileNode {
                            key: key_name.clone(),
                            port: port.take(),
                            peers,
                            dht_config: Default::default(),
                        }],
                        peers: None,
                    };
                    Ok(RunnerConfig::try_from_config(key_storage, config).await?)
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

        let rc = RunnerConfig::try_from_command(
            &env.key_storage,
            CLICommand::Run {
                config: None,
                key: Some(String::from("single-test-key")),
                port: Some(6666),
                bootstrap: None,
            },
        )
        .await?;

        let node_config = rc.nodes.get(0).unwrap();
        assert!(
            keys_equal(&node_config.key_material, &expected_key).await?,
            "expected key material"
        );
        assert_eq!(
            node_config.listening_address,
            "/ip4/127.0.0.1/tcp/6666".to_string().parse()?,
            "expected port"
        );
        assert_eq!(
            node_config.peers.len(),
            1,
            "expected default bootstrap peers"
        );

        assert_eq!(
            node_config.peers.get(0),
            BOOTSTRAP_PEERS[..].get(0),
            "expected default bootstrap peers"
        );

        Ok(())
    }

    #[tokio::test]
    async fn try_from_command_with_config() -> Result<()> {
        let env = Env::new_with_config(
            r#"
peers = [
    "/ip4/127.0.0.1/tcp/9999"
]

[[nodes]]
key = "my-bootstrap-key-1"
port = 10000
peers = [
    "/ip4/127.0.0.1/tcp/10001"
]

[[nodes]]
key = "my-bootstrap-key-2"
port = 20000
"#,
        )
        .await?;

        let key_1 = env.create_key("my-bootstrap-key-1").await?;
        let key_2 = env.create_key("my-bootstrap-key-2").await?;

        let rc = RunnerConfig::try_from_command(
            &env.key_storage,
            CLICommand::Run {
                config: env.config_path.to_owned(),
                key: None,
                port: None,
                bootstrap: None,
            },
        )
        .await?;

        let node_config = rc.nodes.get(0).unwrap();
        assert!(
            keys_equal(&node_config.key_material, &key_1).await?,
            "expected key material"
        );
        assert_eq!(
            node_config.listening_address,
            "/ip4/127.0.0.1/tcp/10000".to_string().parse()?,
            "expected port"
        );
        assert!(
            node_config
                .peers
                .contains(&("/ip4/127.0.0.1/tcp/10001".to_string().parse()?)),
            "expected explicit peer"
        );
        assert!(
            node_config
                .peers
                .contains(&("/ip4/127.0.0.1/tcp/9999".to_string().parse()?)),
            "expected global peer"
        );
        assert_eq!(node_config.peers.len(), 2, "expected 2 peers");

        let node_config = rc.nodes.get(1).unwrap();
        assert!(
            keys_equal(&node_config.key_material, &key_2).await?,
            "expected key material"
        );
        assert_eq!(
            node_config.listening_address,
            "/ip4/127.0.0.1/tcp/20000".to_string().parse()?,
            "expected port"
        );
        assert!(
            node_config
                .peers
                .contains(&("/ip4/127.0.0.1/tcp/9999".to_string().parse()?)),
            "expected global peer"
        );
        assert_eq!(node_config.peers.len(), 1, "expected only global peer");

        Ok(())
    }

    #[tokio::test]
    async fn try_from_command_validation() -> Result<()> {
        let env = Env::new_with_config(
            r#"
[[nodes]]
key = "my-bootstrap-key-1"
port = 10000
[[nodes]]
key = "my-bootstrap-key-2"
port = 20000
"#,
        )
        .await?;

        let commands = [
            CLICommand::Run {
                config: None,
                key: None,
                port: Some(6666),
                bootstrap: None,
            },
            CLICommand::Run {
                config: None,
                key: Some(String::from("key-does-not-exist")),
                port: Some(6666),
                bootstrap: None,
            },
            CLICommand::Run {
                config: env.config_path.to_owned(),
                key: None,
                port: None,
                bootstrap: None,
            },
            CLICommand::Run {
                config: Some(env.dir_path.join("invalid_path")),
                key: None,
                port: None,
                bootstrap: None,
            },
        ];

        for command in commands {
            assert!(
                RunnerConfig::try_from_command(&env.key_storage, command)
                    .await
                    .is_err(),
                "expected failure from try_from_command"
            );
        }

        Ok(())
    }
}
