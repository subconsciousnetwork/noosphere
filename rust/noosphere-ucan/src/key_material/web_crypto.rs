use crate::{
    crypto::{JwtSignatureAlgorithm, KeyMaterial},
    key_material::rsa::RsaKeyMaterial,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use js_sys::{Array, ArrayBuffer, Boolean, Object, Reflect, Uint8Array};
use rsa::{pkcs1::DecodeRsaPublicKey, RsaPublicKey};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Crypto, CryptoKey, CryptoKeyPair, SubtleCrypto};

pub fn convert_spki_to_rsa_public_key(spki_bytes: &[u8]) -> Result<Vec<u8>> {
    // TODO: This is maybe a not-good, hacky solution; verifying the first
    // 24 bytes would be more wholesome
    // SEE: https://github.com/ucan-wg/ts-ucan/issues/30#issuecomment-1007333500
    Ok(Vec::from(&spki_bytes[24..]))
}

pub const WEB_CRYPTO_RSA_ALGORITHM: &str = "RSASSA-PKCS1-v1_5";

#[derive(Debug)]
pub struct WebCryptoRsaKeyMaterial(pub CryptoKey, pub Option<CryptoKey>);

impl WebCryptoRsaKeyMaterial {
    fn get_subtle_crypto() -> Result<SubtleCrypto> {
        // NOTE: Accessing either `Window` or `DedicatedWorkerGlobalScope` in
        // a context where they are not defined will cause a JS error, so we
        // do a sneaky workaround here:
        let global = js_sys::global();
        match Reflect::get(&global, &JsValue::from("crypto")) {
            Ok(value) => Ok(value.dyn_into::<Crypto>().expect("Unexpected API").subtle()),
            _ => Err(anyhow!("Could not access WebCrypto API")),
        }
    }

    fn private_key(&self) -> Result<&CryptoKey> {
        match &self.1 {
            Some(key) => Ok(key),
            None => Err(anyhow!("No private key configured")),
        }
    }

    pub async fn generate(key_size: Option<u32>) -> Result<WebCryptoRsaKeyMaterial> {
        let subtle_crypto = Self::get_subtle_crypto()?;
        let algorithm = Object::new();

        Reflect::set(
            &algorithm,
            &JsValue::from("name"),
            &JsValue::from(WEB_CRYPTO_RSA_ALGORITHM),
        )
        .map_err(|error| anyhow!("{:?}", error))?;

        Reflect::set(
            &algorithm,
            &JsValue::from("modulusLength"),
            &JsValue::from(key_size.unwrap_or(2048)),
        )
        .map_err(|error| anyhow!("{:?}", error))?;

        let public_exponent = Uint8Array::new(&JsValue::from(3u8));
        public_exponent.copy_from(&[0x01u8, 0x00, 0x01]);

        Reflect::set(
            &algorithm,
            &JsValue::from("publicExponent"),
            &JsValue::from(public_exponent),
        )
        .map_err(|error| anyhow!("{:?}", error))?;

        let hash = Object::new();

        Reflect::set(&hash, &JsValue::from("name"), &JsValue::from("SHA-256"))
            .map_err(|error| anyhow!("{:?}", error))?;

        Reflect::set(&algorithm, &JsValue::from("hash"), &JsValue::from(hash))
            .map_err(|error| anyhow!("{:?}", error))?;

        let uses = Array::new();

        uses.push(&JsValue::from("sign"));
        uses.push(&JsValue::from("verify"));

        let crypto_key_pair_generates = subtle_crypto
            .generate_key_with_object(&algorithm, false, &uses)
            .map_err(|error| anyhow!("{:?}", error))?;
        let crypto_key_pair = CryptoKeyPair::from(
            JsFuture::from(crypto_key_pair_generates)
                .await
                .map_err(|error| anyhow!("{:?}", error))?,
        );

        let public_key = CryptoKey::from(
            Reflect::get(&crypto_key_pair, &JsValue::from("publicKey"))
                .map_err(|error| anyhow!("{:?}", error))?,
        );
        let private_key = CryptoKey::from(
            Reflect::get(&crypto_key_pair, &JsValue::from("privateKey"))
                .map_err(|error| anyhow!("{:?}", error))?,
        );

        Ok(WebCryptoRsaKeyMaterial(public_key, Some(private_key)))
    }
}

#[async_trait(?Send)]
impl KeyMaterial for WebCryptoRsaKeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        JwtSignatureAlgorithm::RS256.to_string()
    }

    async fn get_did(&self) -> Result<String> {
        let public_key = &self.0;
        let subtle_crypto = Self::get_subtle_crypto()?;

        let public_key_bytes = Uint8Array::new(
            &JsFuture::from(
                subtle_crypto
                    .export_key("spki", public_key)
                    .expect("Could not access key extraction API"),
            )
            .await
            .expect("Failed to extract public key bytes")
            .dyn_into::<ArrayBuffer>()
            .expect("Bytes were not an ArrayBuffer"),
        );

        let public_key_bytes = public_key_bytes.to_vec();
        let public_key_bytes = convert_spki_to_rsa_public_key(public_key_bytes.as_slice())?;
        let public_key = RsaPublicKey::from_pkcs1_der(&public_key_bytes)?;

        Ok(RsaKeyMaterial(public_key, None).get_did().await?)
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        let key = self.private_key()?;
        let subtle_crypto = Self::get_subtle_crypto()?;
        let algorithm = Object::new();

        Reflect::set(
            &algorithm,
            &JsValue::from("name"),
            &JsValue::from(WEB_CRYPTO_RSA_ALGORITHM),
        )
        .map_err(|error| anyhow!("{:?}", error))?;

        Reflect::set(
            &algorithm,
            &JsValue::from("saltLength"),
            &JsValue::from(128u8),
        )
        .map_err(|error| anyhow!("{:?}", error))?;

        let data = unsafe { Uint8Array::view(payload) };

        let result = Uint8Array::new(
            &JsFuture::from(
                subtle_crypto
                    .sign_with_object_and_buffer_source(&algorithm, key, &data)
                    .map_err(|error| anyhow!("{:?}", error))?,
            )
            .await
            .map_err(|error| anyhow!("{:?}", error))?,
        );

        Ok(result.to_vec())
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        let key = &self.0;
        let subtle_crypto = Self::get_subtle_crypto()?;
        let algorithm = Object::new();

        Reflect::set(
            &algorithm,
            &JsValue::from("name"),
            &JsValue::from(WEB_CRYPTO_RSA_ALGORITHM),
        )
        .map_err(|error| anyhow!("{:?}", error))?;
        Reflect::set(
            &algorithm,
            &JsValue::from("saltLength"),
            &JsValue::from(128u8),
        )
        .map_err(|error| anyhow!("{:?}", error))?;

        let signature = unsafe { Uint8Array::view(signature) };
        let data = unsafe { Uint8Array::view(payload) };

        let valid = JsFuture::from(
            subtle_crypto
                .verify_with_object_and_buffer_source_and_buffer_source(
                    &algorithm, key, &signature, &data,
                )
                .map_err(|error| anyhow!("{:?}", error))?,
        )
        .await
        .map_err(|error| anyhow!("{:?}", error))?
        .dyn_into::<Boolean>()
        .map_err(|error| anyhow!("{:?}", error))?;

        match valid.is_truthy() {
            true => Ok(()),
            false => Err(anyhow!("Could not verify signature")),
        }
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    use super::WebCryptoRsaKeyMaterial;
    use crate::{
        builder::UcanBuilder,
        crypto::{did::DidParser, KeyMaterial},
        key_material::rsa::{bytes_to_rsa_key, RSA_MAGIC_BYTES},
        ucan::Ucan,
    };

    #[wasm_bindgen_test]
    async fn it_can_sign_and_verify_data() {
        let key_material = WebCryptoRsaKeyMaterial::generate(None).await.unwrap();
        let data = &[0xdeu8, 0xad, 0xbe, 0xef];
        let signature = key_material.sign(data).await.unwrap();

        key_material.verify(data, signature.as_ref()).await.unwrap();
    }

    #[wasm_bindgen_test]
    async fn it_produces_a_legible_rsa_did() {
        let key_material = WebCryptoRsaKeyMaterial::generate(None).await.unwrap();
        let did = key_material.get_did().await.unwrap();
        let mut did_parser = DidParser::new(&[(RSA_MAGIC_BYTES, bytes_to_rsa_key)]);

        did_parser.parse(&did).unwrap();
    }

    #[wasm_bindgen_test]
    async fn it_signs_ucans_that_can_be_verified_elsewhere() {
        let key_material = WebCryptoRsaKeyMaterial::generate(None).await.unwrap();

        let token = UcanBuilder::default()
            .issued_by(&key_material)
            .for_audience(key_material.get_did().await.unwrap().as_str())
            .with_lifetime(300)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        let mut did_parser = DidParser::new(&[(RSA_MAGIC_BYTES, bytes_to_rsa_key)]);
        let ucan = Ucan::try_from(token.as_str()).unwrap();

        ucan.check_signature(&mut did_parser).await.unwrap();
    }
}
