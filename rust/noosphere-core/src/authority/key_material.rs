use crate::data::Mnemonic;
use anyhow::{anyhow, Result};
use bip39::{Language, Mnemonic as BipMnemonic};
use ed25519_dalek::{
    SigningKey as Ed25519PrivateKey, VerifyingKey as Ed25519PublicKey, KEYPAIR_LENGTH,
};
use noosphere_ucan::crypto::did::KeyConstructorSlice;
use noosphere_ucan::key_material::{
    ed25519::{bytes_to_ed25519_key, Ed25519KeyMaterial, ED25519_MAGIC_BYTES},
    rsa::{bytes_to_rsa_key, RSA_MAGIC_BYTES},
};

/// A common set of DID Key formats that are supported by this crate
// TODO: Conditional web crypto support
pub const SUPPORTED_KEYS: &KeyConstructorSlice = &[
    (ED25519_MAGIC_BYTES, bytes_to_ed25519_key),
    (RSA_MAGIC_BYTES, bytes_to_rsa_key),
];

/// Produce a unique [Ed25519KeyMaterial] for general purpose use cases
pub fn generate_ed25519_key() -> Ed25519KeyMaterial {
    let mut rng = rand::thread_rng();
    let private_key = Ed25519PrivateKey::generate(&mut rng);
    let public_key = Ed25519PublicKey::from(&private_key);
    Ed25519KeyMaterial(public_key, Some(private_key))
}

/// Restore an [Ed25519KeyMaterial] from a [Mnemonic]
pub fn restore_ed25519_key(mnemonic: &str) -> Result<Ed25519KeyMaterial> {
    let mnemonic = BipMnemonic::from_phrase(mnemonic, Language::English)?;
    let private_key = Ed25519PrivateKey::try_from(mnemonic.entropy())?;
    let public_key = Ed25519PublicKey::from(&private_key);

    Ok(Ed25519KeyMaterial(public_key, Some(private_key)))
}

/// Produce a [Mnemonic] for a given [Ed25519KeyMaterial]; note that the private
/// part of the key must be available in the [Ed25519KeyMaterial] in order to
/// produce the mnemonic.
pub fn ed25519_key_to_mnemonic(key_material: &Ed25519KeyMaterial) -> Result<Mnemonic> {
    match &key_material.1 {
        Some(private_key) => {
            let mnemonic = BipMnemonic::from_entropy(private_key.as_bytes(), Language::English)?;
            Ok(Mnemonic(mnemonic.into_phrase()))
        }
        None => Err(anyhow!(
            "A mnemonic can only be generated for the key material if a private key is configured"
        )),
    }
}

/// For a given [Ed25519KeyMaterial] produce a serialized byte array of the
/// key that is suitable for persisting to secure storage.
pub fn ed25519_key_to_bytes(key_material: &Ed25519KeyMaterial) -> Result<[u8; KEYPAIR_LENGTH]> {
    match &key_material.1 {
        Some(private_key) => Ok(private_key.to_keypair_bytes()),
        None => Err(anyhow!("Private key required in order to deserialize.")),
    }
}
