mod did_from_keypair {
    use base64::Engine;
    use did_key::{from_existing_key, Ed25519KeyPair, KeyMaterial as _KeyMaterial};

    use crate::crypto::KeyMaterial;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_handles_ed25519_keys() {
        let pub_key = base64::engine::general_purpose::STANDARD
            .decode("Hv+AVRD2WUjUFOsSNbsmrp9fokuwrUnjBcr92f0kxw4=")
            .unwrap();
        let key = Ed25519KeyPair::from_public_key(&pub_key);
        let keypair = from_existing_key::<Ed25519KeyPair>(&key.public_key_bytes(), None);

        let expected_did = "did:key:z6MkgYGF3thn8k1Fv4p4dWXKtsXCnLH7q9yw4QgNPULDmDKB";
        let result_did = keypair.get_did().await.unwrap();

        assert_eq!(expected_did, result_did.as_str());
    }
}
