use crate::cli::{CLICommand, CLIConfig, CLIConfigNode};
use crate::utils;
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use noosphere::key::InsecureKeyStorage;
use noosphere_ns::Multiaddr;


use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// DHT node configuration that contains resolved
/// data, ready to instantiate a [DHTNode].
pub struct RunnerNodeConfig {
    pub key_material: Ed25519KeyMaterial,
    pub listening_address: Multiaddr,
}

impl RunnerNodeConfig {
    fn new(key_material: Ed25519KeyMaterial, listening_address: Multiaddr) -> Self {
        RunnerNodeConfig {
            key_material,
            listening_address,
        }
    }

    pub(crate) async fn try_from_key_name(
        key_storage: &InsecureKeyStorage,
        key_name: &str,
        port: u16,
    ) -> Result<Self> {
        let key_material = utils::get_key_material(key_storage, key_name).await?;
        let listening_address = utils::create_listening_address(port);
        Ok(RunnerNodeConfig::new(key_material, listening_address))
    }
}

/// Configuration for [Runner], hydrated/resolved from CLI.
pub struct RunnerConfig {
    pub nodes: Vec<RunnerNodeConfig>,
}

impl RunnerConfig {
    fn new(nodes: Vec<RunnerNodeConfig>) -> Self {
        RunnerConfig { nodes }
    }

    async fn try_from_config(key_storage: &InsecureKeyStorage, config: CLIConfig) -> Result<Self> {
        let futures: Vec<_> = config
            .nodes
            .iter()
            .map(|config_node| {
                let port = config_node.port.unwrap_or(0);
                RunnerNodeConfig::try_from_key_name(key_storage, &config_node.key, port)
            })
            .collect();
        let nodes = try_join_all(futures).await?;
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
                mut port,
            } => match config {
                Some(config_path) => {
                    let toml_str = tokio::fs::read_to_string(&config_path).await?;
                    let config: CLIConfig = toml::from_str(&toml_str)?;
                    Ok(RunnerConfig::try_from_config(key_storage, config).await?)
                }
                None => {
                    let key_name: String =
                        key.ok_or_else(|| anyhow!("--key or --config must be provided."))?;

                    let config = CLIConfig {
                        nodes: vec![CLIConfigNode {
                            key: key_name.clone(),
                            port: port.take(),
                        }],
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

        Ok(())
    }

    #[tokio::test]
    async fn try_from_command_with_config() -> Result<()> {
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

        let key_1 = env.create_key("my-bootstrap-key-1").await?;
        let key_2 = env.create_key("my-bootstrap-key-2").await?;

        let rc = RunnerConfig::try_from_command(
            &env.key_storage,
            CLICommand::Run {
                config: env.config_path.to_owned(),
                key: None,
                port: None,
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
            },
            CLICommand::Run {
                config: None,
                key: Some(String::from("key-does-not-exist")),
                port: Some(6666),
            },
            CLICommand::Run {
                config: env.config_path.to_owned(),
                key: None,
                port: None,
            },
            CLICommand::Run {
                config: Some(env.dir_path.join("invalid_path")),
                key: None,
                port: None,
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
