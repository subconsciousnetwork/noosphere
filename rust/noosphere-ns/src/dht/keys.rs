use anyhow;
use libp2p::identity::Keypair;
use noosphere_core::authority::ed25519_key_to_bytes;
use ucan::crypto::KeyMaterial;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

pub trait DHTKeyMaterial: KeyMaterial + Clone {
    fn to_dht_keypair(&self) -> anyhow::Result<Keypair>;
}

impl DHTKeyMaterial for Ed25519KeyMaterial {
    fn to_dht_keypair(&self) -> anyhow::Result<Keypair> {
        let mut bytes = ed25519_key_to_bytes(self)?;
        let kp = libp2p::identity::ed25519::Keypair::decode(&mut bytes)
            .map_err(|_| anyhow::anyhow!("Could not decode ED25519 key."))?;
        Ok(Keypair::Ed25519(kp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p;
    use noosphere_core::authority::generate_ed25519_key;

    #[test]
    fn it_converts_to_libp2p_keypair() -> anyhow::Result<()> {
        let zebra_keys = generate_ed25519_key();
        let libp2p::identity::Keypair::Ed25519(keypair) = zebra_keys.to_dht_keypair()?;
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
