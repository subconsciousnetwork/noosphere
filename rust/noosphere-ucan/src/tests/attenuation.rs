use super::fixtures::{EmailSemantics, Identities, SUPPORTED_KEYS};
use crate::{
    builder::UcanBuilder,
    capability::{Capability, CapabilitySemantics},
    chain::{CapabilityInfo, ProofChain},
    crypto::did::DidParser,
    store::{MemoryStore, UcanJwtStore},
};
use std::collections::BTreeSet;

use serde_json::json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_works_with_a_simple_example() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let email_semantics = EmailSemantics {};
    let send_email_as_alice = email_semantics
        .parse("mailto:alice@email.com", "email/send", None)
        .unwrap();

    let leaf_ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .claiming_capability(&send_email_as_alice)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let attenuated_token = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, None)
        .claiming_capability(&send_email_as_alice)
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
        ProofChain::try_from_token_string(attenuated_token.as_str(), None, &mut did_parser, &store)
            .await
            .unwrap();

    let capability_infos = chain.reduce_capabilities(&email_semantics);

    assert_eq!(capability_infos.len(), 1);

    let info = capability_infos.get(0).unwrap();

    assert_eq!(
        info.capability.resource().to_string().as_str(),
        "mailto:alice@email.com",
    );
    assert_eq!(info.capability.ability().to_string().as_str(), "email/send");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_reports_the_first_issuer_in_the_chain_as_originator() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let email_semantics = EmailSemantics {};
    let send_email_as_bob = email_semantics
        .parse("mailto:bob@email.com".into(), "email/send".into(), None)
        .unwrap();

    let leaf_ucan = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.bob_did.as_str())
        .with_lifetime(60)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let ucan_token = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan, None)
        .claiming_capability(&send_email_as_bob)
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

    let capability_infos =
        ProofChain::try_from_token_string(&ucan_token, None, &mut did_parser, &store)
            .await
            .unwrap()
            .reduce_capabilities(&email_semantics);

    assert_eq!(capability_infos.len(), 1);

    let info = capability_infos.get(0).unwrap();

    assert_eq!(
        info.originators.iter().collect::<Vec<&String>>(),
        vec![&identities.bob_did]
    );
    assert_eq!(info.capability, send_email_as_bob);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_finds_the_right_proof_chain_for_the_originator() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let email_semantics = EmailSemantics {};
    let send_email_as_bob = email_semantics
        .parse("mailto:bob@email.com".into(), "email/send".into(), None)
        .unwrap();
    let send_email_as_alice = email_semantics
        .parse("mailto:alice@email.com".into(), "email/send".into(), None)
        .unwrap();

    let leaf_ucan_alice = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(60)
        .claiming_capability(&send_email_as_alice)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let leaf_ucan_bob = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(60)
        .claiming_capability(&send_email_as_bob)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let ucan = UcanBuilder::default()
        .issued_by(&identities.mallory_key)
        .for_audience(identities.alice_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan_alice, None)
        .witnessed_by(&leaf_ucan_bob, None)
        .claiming_capability(&send_email_as_alice)
        .claiming_capability(&send_email_as_bob)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let ucan_token = ucan.encode().unwrap();

    let mut store = MemoryStore::default();
    store
        .write_token(&leaf_ucan_alice.encode().unwrap())
        .await
        .unwrap();
    store
        .write_token(&leaf_ucan_bob.encode().unwrap())
        .await
        .unwrap();

    let proof_chain = ProofChain::try_from_token_string(&ucan_token, None, &mut did_parser, &store)
        .await
        .unwrap();
    let capability_infos = proof_chain.reduce_capabilities(&email_semantics);

    assert_eq!(capability_infos.len(), 2);

    let send_email_as_bob_info = capability_infos.get(0).unwrap();
    let send_email_as_alice_info = capability_infos.get(1).unwrap();

    assert_eq!(
        send_email_as_alice_info,
        &CapabilityInfo {
            originators: BTreeSet::from_iter(vec![identities.alice_did]),
            capability: send_email_as_alice,
            not_before: ucan.not_before().clone(),
            expires_at: ucan.expires_at().clone()
        }
    );

    assert_eq!(
        send_email_as_bob_info,
        &CapabilityInfo {
            originators: BTreeSet::from_iter(vec![identities.bob_did]),
            capability: send_email_as_bob,
            not_before: ucan.not_before().clone(),
            expires_at: ucan.expires_at().clone()
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_reports_all_chain_options() {
    let identities = Identities::new().await;
    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    let email_semantics = EmailSemantics {};
    let send_email_as_alice = email_semantics
        .parse("mailto:alice@email.com".into(), "email/send".into(), None)
        .unwrap();

    let leaf_ucan_alice = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(60)
        .claiming_capability(&send_email_as_alice)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let leaf_ucan_bob = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_lifetime(60)
        .claiming_capability(&send_email_as_alice)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let ucan = UcanBuilder::default()
        .issued_by(&identities.mallory_key)
        .for_audience(identities.alice_did.as_str())
        .with_lifetime(50)
        .witnessed_by(&leaf_ucan_alice, None)
        .witnessed_by(&leaf_ucan_bob, None)
        .claiming_capability(&send_email_as_alice)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let ucan_token = ucan.encode().unwrap();

    let mut store = MemoryStore::default();
    store
        .write_token(&leaf_ucan_alice.encode().unwrap())
        .await
        .unwrap();
    store
        .write_token(&leaf_ucan_bob.encode().unwrap())
        .await
        .unwrap();

    let proof_chain = ProofChain::try_from_token_string(&ucan_token, None, &mut did_parser, &store)
        .await
        .unwrap();
    let capability_infos = proof_chain.reduce_capabilities(&email_semantics);

    assert_eq!(capability_infos.len(), 1);

    let send_email_as_alice_info = capability_infos.get(0).unwrap();

    assert_eq!(
        send_email_as_alice_info,
        &CapabilityInfo {
            originators: BTreeSet::from_iter(vec![identities.alice_did, identities.bob_did]),
            capability: send_email_as_alice,
            not_before: ucan.not_before().clone(),
            expires_at: ucan.expires_at().clone()
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
pub async fn it_validates_caveats() -> anyhow::Result<()> {
    let resource = "mailto:alice@email.com";
    let ability = "email/send";

    let no_caveat = Capability::from((resource, ability, &json!({})));
    let x_caveat = Capability::from((resource, ability, &json!({ "x": true })));
    let y_caveat = Capability::from((resource, ability, &json!({ "y": true })));
    let z_caveat = Capability::from((resource, ability, &json!({ "z": true })));
    let yz_caveat = Capability::from((resource, ability, &json!({ "y": true, "z": true })));

    let valid = [
        (vec![&no_caveat], vec![&no_caveat]),
        (vec![&x_caveat], vec![&x_caveat]),
        (vec![&no_caveat], vec![&x_caveat]),
        (vec![&x_caveat, &y_caveat], vec![&x_caveat]),
        (vec![&x_caveat, &y_caveat], vec![&x_caveat, &yz_caveat]),
    ];

    let invalid = [
        (vec![&x_caveat], vec![&no_caveat]),
        (vec![&x_caveat], vec![&y_caveat]),
        (
            vec![&x_caveat, &y_caveat],
            vec![&x_caveat, &y_caveat, &z_caveat],
        ),
    ];

    for (proof_capabilities, delegated_capabilities) in valid {
        let is_successful =
            test_capabilities_delegation(&proof_capabilities, &delegated_capabilities).await?;
        assert!(
            is_successful,
            "{} enables {}",
            render_caveats(&proof_capabilities),
            render_caveats(&delegated_capabilities)
        );
    }

    for (proof_capabilities, delegated_capabilities) in invalid {
        let is_successful =
            test_capabilities_delegation(&proof_capabilities, &delegated_capabilities).await?;
        assert!(
            !is_successful,
            "{} disallows {}",
            render_caveats(&proof_capabilities),
            render_caveats(&delegated_capabilities)
        );
    }

    fn render_caveats(capabilities: &Vec<&Capability>) -> String {
        format!(
            "{:?}",
            capabilities
                .iter()
                .map(|cap| cap.caveat.to_string())
                .collect::<Vec<String>>()
        )
    }

    async fn test_capabilities_delegation(
        proof_capabilities: &Vec<&Capability>,
        delegated_capabilities: &Vec<&Capability>,
    ) -> anyhow::Result<bool> {
        let identities = Identities::new().await;
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let email_semantics = EmailSemantics {};
        let mut store = MemoryStore::default();
        let proof_capabilities = proof_capabilities
            .to_owned()
            .into_iter()
            .map(|cap| cap.to_owned())
            .collect::<Vec<Capability>>();
        let delegated_capabilities = delegated_capabilities
            .to_owned()
            .into_iter()
            .map(|cap| cap.to_owned())
            .collect::<Vec<Capability>>();

        let proof_ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.mallory_did.as_str())
            .with_lifetime(60)
            .claiming_capabilities(&proof_capabilities)
            .build()?
            .sign()
            .await?;

        let ucan = UcanBuilder::default()
            .issued_by(&identities.mallory_key)
            .for_audience(identities.alice_did.as_str())
            .with_lifetime(50)
            .witnessed_by(&proof_ucan, None)
            .claiming_capabilities(&delegated_capabilities)
            .build()?
            .sign()
            .await?;
        store.write_token(&proof_ucan.encode().unwrap()).await?;
        store.write_token(&ucan.encode().unwrap()).await?;

        let proof_chain = ProofChain::from_ucan(ucan, None, &mut did_parser, &store).await?;

        Ok(enables_capabilities(
            &proof_chain,
            &email_semantics,
            &identities.alice_did,
            &delegated_capabilities,
        ))
    }

    /// Checks proof chain returning true if all desired capabilities are enabled.
    fn enables_capabilities(
        proof_chain: &ProofChain,
        semantics: &EmailSemantics,
        originator: &String,
        desired_capabilities: &Vec<Capability>,
    ) -> bool {
        let capability_infos = proof_chain.reduce_capabilities(semantics);

        for desired_capability in desired_capabilities {
            let mut has_capability = false;
            for info in &capability_infos {
                if info.originators.contains(originator)
                    && info
                        .capability
                        .enables(&semantics.parse_capability(desired_capability).unwrap())
                {
                    has_capability = true;
                    break;
                }
            }
            if !has_capability {
                return false;
            }
        }
        true
    }

    Ok(())
}
