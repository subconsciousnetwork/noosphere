use anyhow::{anyhow, Result};
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
