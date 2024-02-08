use anyhow::{anyhow, Result};
use async_trait::async_trait;

use p256::ecdsa::{
    self,
    signature::{Signer, Verifier},
    Signature, SigningKey as P256PrivateKey, VerifyingKey as P256PublicKey,
};

use noosphere_ucan::crypto::KeyMaterial;

pub use noosphere_ucan::crypto::{did::P256_MAGIC_BYTES, JwtSignatureAlgorithm};

pub fn bytes_to_p256_key(bytes: Vec<u8>) -> Result<Box<dyn KeyMaterial>> {
    let public_key = P256PublicKey::try_from(bytes.as_slice())?;
    Ok(Box::new(P256KeyMaterial(public_key, None)))
}

/// Support for NIST P-256 keys, aka secp256r1, aka ES256
#[derive(Clone)]
pub struct P256KeyMaterial(pub P256PublicKey, pub Option<P256PrivateKey>);

#[cfg_attr(target_arch="wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl KeyMaterial for P256KeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        JwtSignatureAlgorithm::ES256.to_string()
    }

    async fn get_did(&self) -> Result<String> {
        let bytes = [P256_MAGIC_BYTES, &self.0.to_encoded_point(true).to_bytes()].concat();
        Ok(format!("did:key:z{}", bs58::encode(bytes).into_string()))
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        match self.1 {
            Some(ref private_key) => {
                let signature: ecdsa::Signature = private_key.sign(payload);
                Ok(signature.to_vec())
            }
            None => Err(anyhow!("No private key; cannot sign data")),
        }
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        let signature = Signature::try_from(signature)?;
        self.0
            .verify(payload, &signature)
            .map_err(|error| anyhow!("Could not verify signature: {:?}", error))
    }
}

#[cfg(test)]
mod tests {
    use super::{bytes_to_p256_key, P256KeyMaterial, P256_MAGIC_BYTES};
    use noosphere_ucan::{
        builder::UcanBuilder,
        crypto::{did::DidParser, KeyMaterial},
        ucan::Ucan,
    };
    use p256::ecdsa::{SigningKey as P256PrivateKey, VerifyingKey as P256PublicKey};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_sign_and_verify_a_ucan() {
        let private_key = P256PrivateKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let public_key = P256PublicKey::from(&private_key);

        let key_material = P256KeyMaterial(public_key, Some(private_key));
        let token_string = UcanBuilder::default()
            .issued_by(&key_material)
            .for_audience(key_material.get_did().await.unwrap().as_str())
            .with_lifetime(60)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        let mut did_parser = DidParser::new(&[(P256_MAGIC_BYTES, bytes_to_p256_key)]);

        let ucan = Ucan::try_from(token_string).unwrap();
        ucan.check_signature(&mut did_parser).await.unwrap();
    }
}
