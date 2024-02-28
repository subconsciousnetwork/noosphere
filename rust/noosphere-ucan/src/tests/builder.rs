use crate::{
    builder::UcanBuilder,
    capability::{Capabilities, Capability, CapabilitySemantics},
    chain::ProofChain,
    crypto::did::DidParser,
    key_material::ed25519::Ed25519KeyMaterial,
    store::UcanJwtStore,
    tests::fixtures::{
        Blake2bMemoryStore, EmailSemantics, Identities, WNFSSemantics, SUPPORTED_KEYS,
    },
    time::now,
};
use cid::multihash::Code;
use serde_json::json;
use std::collections::BTreeMap;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn it_builds_with_a_simple_example() {
    let identities = Identities::new().await;

    let fact_1 = json!({
        "test": true
    });

    let fact_2 = json!({
        "preimage": "abc",
        "hash": "sth"
    });

    let email_semantics = EmailSemantics {};
    let wnfs_semantics = WNFSSemantics {};

    let cap_1 = email_semantics
        .parse("mailto:alice@gmail.com", "email/send", None)
        .unwrap();

    let cap_2 = wnfs_semantics
        .parse("wnfs://alice.fission.name/public", "wnfs/super_user", None)
        .unwrap();

    let expiration = now() + 30;
    let not_before = now() - 30;

    let token = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_expiration(expiration)
        .not_before(not_before)
        .with_fact("abc/challenge", fact_1.clone())
        .with_fact("def/challenge", fact_2.clone())
        .claiming_capability(&cap_1)
        .claiming_capability(&cap_2)
        .with_nonce()
        .build()
        .unwrap();

    let ucan = token.sign().await.unwrap();

    assert_eq!(ucan.issuer(), identities.alice_did);
    assert_eq!(ucan.audience(), identities.bob_did);
    assert!(ucan.expires_at().is_some());
    assert_eq!(ucan.expires_at().unwrap(), expiration);
    assert!(ucan.not_before().is_some());
    assert_eq!(ucan.not_before().unwrap(), not_before);
    assert_eq!(
        ucan.facts(),
        &Some(BTreeMap::from([
            (String::from("abc/challenge"), fact_1),
            (String::from("def/challenge"), fact_2),
        ]))
    );

    let expected_attenuations =
        Capabilities::try_from(vec![Capability::from(&cap_1), Capability::from(&cap_2)]).unwrap();

    assert_eq!(ucan.capabilities(), &expected_attenuations);
    assert!(ucan.nonce().is_some());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn it_builds_with_lifetime_in_seconds() {
    let identities = Identities::new().await;

    let ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(300)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    assert!(ucan.expires_at().unwrap() > (now() + 290));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn it_prevents_duplicate_proofs() {
    let wnfs_semantics = WNFSSemantics {};

    let parent_cap = wnfs_semantics
        .parse("wnfs://alice.fission.name/public", "wnfs/super_user", None)
        .unwrap();

    let identities = Identities::new().await;
    let ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(30)
        .claiming_capability(&parent_cap)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let attenuated_cap_1 = wnfs_semantics
        .parse("wnfs://alice.fission.name/public/Apps", "wnfs/create", None)
        .unwrap();

    let attenuated_cap_2 = wnfs_semantics
        .parse(
            "wnfs://alice.fission.name/public/Domains",
            "wnfs/create",
            None,
        )
        .unwrap();

    let next_ucan = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(30)
        .witnessed_by(&ucan, None)
        .claiming_capability(&attenuated_cap_1)
        .claiming_capability(&attenuated_cap_2)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    assert_eq!(
        next_ucan.proofs(),
        &Some(vec![ucan
            .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
            .unwrap()
            .to_string()])
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_can_use_custom_hasher() {
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
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, Some(Code::Blake2b256))
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let mut store = Blake2bMemoryStore::default();

    store
        .write_token(&leaf_ucan.encode().unwrap())
        .await
        .unwrap();

    let _ = store
        .write_token(&delegated_token.encode().unwrap())
        .await
        .unwrap();

    let valid_chain =
        ProofChain::from_ucan(delegated_token, Some(now()), &mut did_parser, &store).await;

    assert!(valid_chain.is_ok());
}
