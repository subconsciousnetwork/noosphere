use crate::data::Mnemonic;
use anyhow::{anyhow, Result};
use bip39::{Language, Mnemonic as BipMnemonic};
use ed25519_zebra::{SigningKey as Ed25519PrivateKey, VerificationKey as Ed25519PublicKey};
use ucan::crypto::did::KeyConstructorSlice;
use ucan_key_support::{
    ed25519::{bytes_to_ed25519_key, Ed25519KeyMaterial, ED25519_MAGIC_BYTES},
    rsa::{bytes_to_rsa_key, RSA_MAGIC_BYTES},
};

// TODO: Conditional web crypto support
pub const SUPPORTED_KEYS: &KeyConstructorSlice = &[
    (ED25519_MAGIC_BYTES, bytes_to_ed25519_key),
    (RSA_MAGIC_BYTES, bytes_to_rsa_key),
];

pub fn generate_ed25519_key() -> Ed25519KeyMaterial {
    let private_key = Ed25519PrivateKey::new(rand::thread_rng());
    let public_key = Ed25519PublicKey::from(&private_key);
    Ed25519KeyMaterial(public_key, Some(private_key))
}

pub fn restore_ed25519_key(mnemonic: &str) -> Result<Ed25519KeyMaterial> {
    let mnemonic = BipMnemonic::from_phrase(mnemonic, Language::English)?;
    let private_key = Ed25519PrivateKey::try_from(mnemonic.entropy())?;
    let public_key = Ed25519PublicKey::try_from(&private_key)?;

    Ok(Ed25519KeyMaterial(public_key, Some(private_key)))
}

pub fn ed25519_key_to_mnemonic(key_material: &Ed25519KeyMaterial) -> Result<Mnemonic> {
    let private_key = &key_material.1.ok_or_else(|| {
        anyhow!(
            "A mnemonic can only be generated for the key material if a private key is configured"
        )
    })?;
    let mnemonic = BipMnemonic::from_entropy(private_key.as_ref(), Language::English)?;
    Ok(Mnemonic(mnemonic.into_phrase()))
}

pub const ED25519_KEYPAIR_LENGTH: usize = 64;
pub const ED25519_KEY_LENGTH: usize = 32;
pub fn ed25519_key_to_bytes(
    key_material: &Ed25519KeyMaterial,
) -> Result<[u8; ED25519_KEYPAIR_LENGTH]> {
    let public_key = key_material.0;
    let private_key: Ed25519PrivateKey = key_material
        .1
        .ok_or_else(|| anyhow!("Private key required in order to deserialize."))?;

    let mut bytes: [u8; ED25519_KEYPAIR_LENGTH] = [0u8; ED25519_KEYPAIR_LENGTH];
    bytes[..ED25519_KEY_LENGTH].copy_from_slice(private_key.as_ref());
    bytes[ED25519_KEY_LENGTH..].copy_from_slice(public_key.as_ref());
    Ok(bytes)
}
