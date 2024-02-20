use super::KeyMaterial;
use anyhow::{anyhow, Result};
use std::{collections::BTreeMap, sync::Arc};

pub type DidPrefix = &'static [u8];
pub type BytesToKey = fn(Vec<u8>) -> Result<Box<dyn KeyMaterial>>;
pub type KeyConstructors = BTreeMap<DidPrefix, BytesToKey>;
pub type KeyConstructorSlice = [(DidPrefix, BytesToKey)];
pub type KeyCache = BTreeMap<String, Arc<Box<dyn KeyMaterial>>>;

pub const DID_PREFIX: &str = "did:";
pub const DID_KEY_PREFIX: &str = "did:key:z";

pub const ED25519_MAGIC_BYTES: &[u8] = &[0xed, 0x01];
pub const RSA_MAGIC_BYTES: &[u8] = &[0x85, 0x24];
pub const BLS12381G1_MAGIC_BYTES: &[u8] = &[0xea, 0x01];
pub const BLS12381G2_MAGIC_BYTES: &[u8] = &[0xeb, 0x01];
pub const P256_MAGIC_BYTES: &[u8] = &[0x80, 0x24];
pub const SECP256K1_MAGIC_BYTES: &[u8] = &[0xe7, 0x1];

/// A parser that is able to convert from a DID string into a corresponding
/// [`KeyMaterial`] implementation. The parser extracts the signature
/// magic bytes from a given DID and tries to match them to a corresponding
/// constructor function that produces a `SigningKey`.
pub struct DidParser {
    key_constructors: KeyConstructors,
    key_cache: KeyCache,
}

impl DidParser {
    pub fn new(key_constructor_slice: &KeyConstructorSlice) -> Self {
        let mut key_constructors = BTreeMap::new();
        for pair in key_constructor_slice {
            key_constructors.insert(pair.0, pair.1);
        }
        DidParser {
            key_constructors,
            key_cache: BTreeMap::new(),
        }
    }

    pub fn parse(&mut self, did: &str) -> Result<Arc<Box<dyn KeyMaterial>>> {
        if !did.starts_with(DID_KEY_PREFIX) {
            return Err(anyhow!("Expected valid did:key, got: {}", did));
        }

        let did = did.to_owned();
        if let Some(key) = self.key_cache.get(&did) {
            return Ok(key.clone());
        }

        let did_bytes = bs58::decode(&did[DID_KEY_PREFIX.len()..]).into_vec()?;
        let magic_bytes = &did_bytes[0..2];
        match self.key_constructors.get(magic_bytes) {
            Some(ctor) => {
                let key = ctor(Vec::from(&did_bytes[2..]))?;
                self.key_cache.insert(did.clone(), Arc::new(key));

                self.key_cache
                    .get(&did)
                    .ok_or_else(|| anyhow!("Couldn't find cached key"))
                    .cloned()
            }
            None => Err(anyhow!("Unrecognized magic bytes: {:?}", magic_bytes)),
        }
    }
}
