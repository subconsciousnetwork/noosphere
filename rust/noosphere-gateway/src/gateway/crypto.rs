use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere::authority::{ed25519_key_to_mnemonic, generate_ed25519_key, restore_ed25519_key};
use ucan::crypto::KeyMaterial;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

use crate::gateway::environment::GatewayConfig;

pub enum GatewayKey {
    Bare(Ed25519KeyMaterial),
    Secure { name: String },
}

impl GatewayKey {
    pub async fn initialize(config: &mut GatewayConfig) -> Result<GatewayKey> {
        match config.get_hardware_enclave_mode().await? {
            None => {
                warn!("No hardware enclave or TPM configured; gateway private key material will be stored in clear text!");
                match config.get_insecure_mnemonic().await? {
                    Some(mnemonic) => Ok(GatewayKey::Bare(restore_ed25519_key(&mnemonic)?)),
                    None => {
                        let key = generate_ed25519_key();
                        let mnemonic = ed25519_key_to_mnemonic(&key)?;

                        config.set_insecure_mnemonic(&mnemonic).await?;

                        Ok(GatewayKey::Bare(key))
                    }
                }
            }
            _ => todo!("#6: Implement TPM support"),
        }
    }
}

#[async_trait]
impl KeyMaterial for GatewayKey {
    fn get_jwt_algorithm_name(&self) -> String {
        match self {
            GatewayKey::Bare(key) => key.get_jwt_algorithm_name(),
            GatewayKey::Secure { .. } => todo!("#6: Implement TPM support"),
        }
    }

    async fn get_did(&self) -> Result<String> {
        match self {
            GatewayKey::Bare(key) => key.get_did().await,
            GatewayKey::Secure { .. } => todo!("#6: Implement TPM support"),
        }
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        match self {
            GatewayKey::Bare(key) => key.sign(payload).await,
            GatewayKey::Secure { .. } => todo!("#6: Implement TPM support"),
        }
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        match self {
            GatewayKey::Bare(key) => key.verify(payload, signature).await,
            GatewayKey::Secure { .. } => todo!("#6: Implement TPM support"),
        }
    }
}
