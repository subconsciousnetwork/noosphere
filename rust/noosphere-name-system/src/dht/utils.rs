use anyhow::{anyhow, Result};
use libp2p::multihash::MultihashDigest;
use noosphere::authority::ed25519_key_to_bytes;
/// @TODO these materials should be exposed in noosphere::authority
use ucan_key_support::ed25519::Ed25519KeyMaterial;

pub fn key_material_to_libp2p_keypair(
    key_material: &Ed25519KeyMaterial,
) -> Result<libp2p::identity::Keypair> {
    let mut bytes = ed25519_key_to_bytes(key_material)?;
    let kp = libp2p::identity::ed25519::Keypair::decode(&mut bytes)
        .map_err(|_| anyhow!("Could not decode ED25519 key."))?;
    Ok(libp2p::identity::Keypair::Ed25519(kp))
}

/// [PeerId]s are generated from a public key. If this key is less
/// than 42 bytes after being encoded into protobuf form, then
/// an "Identity" format is used rather than the SHA2_256 hash.
/// This saves bytes, but results in a PeerId that does not
/// begin with "Qm". This function forces using SHA2_256.
/// This is unused currently, exploring if everything is supported
/// as "Identity" format, which the KeyMaterial keys we use currently
/// are encoded as.
/// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#peer-ids
pub fn peer_id_from_key_with_sha256(
    public_key: &libp2p::identity::PublicKey,
) -> Result<libp2p::PeerId> {
    let encoded = public_key.to_protobuf_encoding();
    let mh = libp2p::multihash::Code::Sha2_256.digest(&encoded);
    libp2p::PeerId::from_multihash(mh).map_err(|_| anyhow!("nope"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere::authority::generate_ed25519_key;
    #[test]
    fn testkey_material_to_libp2p_keypair() -> Result<()> {
        let zebra_keys = generate_ed25519_key();
        let keypair: libp2p::identity::ed25519::Keypair =
            match key_material_to_libp2p_keypair(&zebra_keys) {
                Ok(kp) => match kp {
                    libp2p::identity::Keypair::Ed25519(keypair) => Ok(keypair),
                    _ => Err(anyhow!("Invalid keypair variant")),
                },
                Err(e) => Err(e),
            }?;
        let zebra_private_key = zebra_keys.1.expect("Has private key");
        let dalek_public_key = keypair.public().encode();
        let dalek_private_key = keypair.secret();

        let in_public_key = zebra_keys.0.as_ref();
        let in_private_key = zebra_private_key.as_ref();
        let out_public_key = dalek_public_key.as_ref();
        let out_private_key = dalek_private_key.as_ref();
        assert_eq!(in_public_key, out_public_key);
        assert_eq!(in_private_key, out_private_key);
        Ok(())
    }
}
