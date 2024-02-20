use super::fixtures::{Identities, SUPPORTED_KEYS};
use crate::{
    builder::UcanBuilder,
    chain::ProofChain,
    crypto::did::DidParser,
    store::{MemoryStore, UcanJwtStore},
    time::now,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_decodes_deep_ucan_chains() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let leaf_ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let delegated_token = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, None)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap()
        .encode()
        .unwrap();

    let mut store = MemoryStore::default();
    store
        .write_token(&leaf_ucan.encode().unwrap())
        .await
        .unwrap();

    let chain =
        ProofChain::try_from_token_string(delegated_token.as_str(), None, &mut did_parser, &store)
            .await
            .unwrap();

    assert_eq!(chain.ucan().audience(), &identities.mallory_did);
    assert_eq!(
        chain.proofs().get(0).unwrap().ucan().issuer(),
        &identities.alice_did
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_fails_with_incorrect_chaining() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let leaf_ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let delegated_token = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, None)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap()
        .encode()
        .unwrap();

    let mut store = MemoryStore::default();
    store
        .write_token(&leaf_ucan.encode().unwrap())
        .await
        .unwrap();

    let parse_token_result =
        ProofChain::try_from_token_string(delegated_token.as_str(), None, &mut did_parser, &store)
            .await;

    assert!(parse_token_result.is_err());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_can_be_instantiated_by_cid() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let leaf_ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let delegated_token = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, None)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap()
        .encode()
        .unwrap();

    let mut store = MemoryStore::default();

    store
        .write_token(&leaf_ucan.encode().unwrap())
        .await
        .unwrap();

    let cid = store.write_token(&delegated_token).await.unwrap();

    let chain = ProofChain::from_cid(&cid, None, &mut did_parser, &store)
        .await
        .unwrap();

    assert_eq!(chain.ucan().audience(), &identities.mallory_did);
    assert_eq!(
        chain.proofs().get(0).unwrap().ucan().issuer(),
        &identities.alice_did
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_can_handle_multiple_leaves() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let leaf_ucan_1 = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let leaf_ucan_2 = UcanBuilder::default()
        .issued_by(&identities.mallory_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let delegated_token = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.alice_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan_1, None)
        .witnessed_by(&leaf_ucan_2, None)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap()
        .encode()
        .unwrap();

    let mut store = MemoryStore::default();
    store
        .write_token(&leaf_ucan_1.encode().unwrap())
        .await
        .unwrap();
    store
        .write_token(&leaf_ucan_2.encode().unwrap())
        .await
        .unwrap();

    ProofChain::try_from_token_string(&delegated_token, None, &mut did_parser, &store)
        .await
        .unwrap();
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_can_use_a_custom_timestamp_to_validate_a_ucan() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let leaf_ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let delegated_token = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, None)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap()
        .encode()
        .unwrap();

    let mut store = MemoryStore::default();

    store
        .write_token(&leaf_ucan.encode().unwrap())
        .await
        .unwrap();

    let cid = store.write_token(&delegated_token).await.unwrap();

    let valid_chain = ProofChain::from_cid(&cid, Some(now()), &mut did_parser, &store).await;

    assert!(valid_chain.is_ok());

    let invalid_chain = ProofChain::from_cid(&cid, Some(now() + 61), &mut did_parser, &store).await;

    assert!(invalid_chain.is_err());
}
