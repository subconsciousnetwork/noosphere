use crate::crypto::{JwtSignatureAlgorithm, KeyMaterial};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rsa::{
    pkcs1::{DecodeRsaPublicKey, EncodeRsaPublicKey},
    Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey,
};
use sha2::{Digest, Sha256};

pub use crate::crypto::did::RSA_MAGIC_BYTES;

pub fn bytes_to_rsa_key(bytes: Vec<u8>) -> Result<Box<dyn KeyMaterial>> {
    println!("Trying to parse RSA key...");
    // NOTE: DID bytes are PKCS1, but we store RSA keys as PKCS8
    let public_key = RsaPublicKey::from_pkcs1_der(&bytes)?;

    Ok(Box::new(RsaKeyMaterial(public_key, None)))
}

#[derive(Clone)]
pub struct RsaKeyMaterial(pub RsaPublicKey, pub Option<RsaPrivateKey>);

#[cfg_attr(target_arch="wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl KeyMaterial for RsaKeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        JwtSignatureAlgorithm::RS256.to_string()
    }

    async fn get_did(&self) -> Result<String> {
        let bytes = match self.0.to_pkcs1_der() {
            Ok(document) => [RSA_MAGIC_BYTES, document.as_bytes()].concat(),
            Err(error) => {
                // TODO: Probably shouldn't swallow this error...
                tracing::warn!("Could not get RSA public key bytes for DID: {:?}", error);
                Vec::new()
            }
        };
        Ok(format!("did:key:z{}", bs58::encode(bytes).into_string()))
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hashed = hasher.finalize();

        match &self.1 {
            Some(private_key) => {
                let padding = Pkcs1v15Sign::new::<Sha256>();
                let signature = private_key.sign(padding, hashed.as_ref())?;
                tracing::info!("SIGNED!");
                Ok(signature)
            }
            None => Err(anyhow!("No private key; cannot sign data")),
        }
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hashed = hasher.finalize();
        let padding = Pkcs1v15Sign::new::<Sha256>();

        self.0
            .verify(padding, hashed.as_ref(), signature)
            .map_err(|error| anyhow!(error))
    }
}

#[cfg(test)]
mod tests {
    use super::{bytes_to_rsa_key, RsaKeyMaterial, RSA_MAGIC_BYTES};
    use crate::{
        builder::UcanBuilder,
        crypto::{did::DidParser, KeyMaterial},
        ucan::Ucan,
    };
    use rsa::{pkcs8::DecodePrivateKey, RsaPrivateKey, RsaPublicKey};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_sign_and_verify_a_ucan() {
        let private_key =
            RsaPrivateKey::from_pkcs8_der(include_bytes!("./fixtures/rsa_key.pk8")).unwrap();
        let public_key = RsaPublicKey::from(&private_key);

        let key_material = RsaKeyMaterial(public_key, Some(private_key));
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

        let mut did_parser = DidParser::new(&[(RSA_MAGIC_BYTES, bytes_to_rsa_key)]);

        let ucan = Ucan::try_from(token_string).unwrap();
        ucan.check_signature(&mut did_parser).await.unwrap();
    }
}
