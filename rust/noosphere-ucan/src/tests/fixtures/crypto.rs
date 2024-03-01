use crate::{
    crypto::did::{KeyConstructorSlice, ED25519_MAGIC_BYTES},
    key_material::ed25519::bytes_to_ed25519_key,
};

pub const SUPPORTED_KEYS: &KeyConstructorSlice = &[
    // https://github.com/multiformats/multicodec/blob/e9ecf587558964715054a0afcc01f7ace220952c/table.csv#L94
    (ED25519_MAGIC_BYTES, bytes_to_ed25519_key),
];
