mod validate {
    use crate::{
        builder::UcanBuilder,
        capability::CapabilitySemantics,
        crypto::did::DidParser,
        tests::fixtures::{EmailSemantics, Identities, SUPPORTED_KEYS},
        time::now,
        ucan::Ucan,
    };
    use anyhow::Result;

    use serde_json::json;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_round_trips_with_encode() {
        let identities = Identities::new().await;
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);

        let ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .with_lifetime(30)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        let encoded_ucan = ucan.encode().unwrap();
        let decoded_ucan = Ucan::try_from(encoded_ucan.as_str()).unwrap();

        decoded_ucan.validate(None, &mut did_parser).await.unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_identifies_a_ucan_that_is_not_active_yet() {
        let identities = Identities::new().await;

        let ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .not_before(now() + 30)
            .with_lifetime(30)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        assert!(ucan.is_too_early());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_identifies_a_ucan_that_has_become_active() {
        let identities = Identities::new().await;
        let ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .not_before(now() / 1000)
            .with_lifetime(30)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        assert!(!ucan.is_too_early());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_serialized_as_json() -> Result<()> {
        let identities = Identities::new().await;

        let email_semantics = EmailSemantics {};
        let send_email_as_alice = email_semantics
            .parse("mailto:alice@email.com".into(), "email/send".into(), None)
            .unwrap();

        let ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .not_before(now() / 1000)
            .with_lifetime(30)
            .with_fact("abc/challenge", json!({ "foo": "bar" }))
            .claiming_capability(&send_email_as_alice)
            .build()?
            .sign()
            .await?;

        let ucan_json = serde_json::to_value(ucan.clone())?;

        assert_eq!(
            ucan_json,
            serde_json::json!({
                "header": {
                    "alg": "EdDSA",
                    "typ": "JWT"
                },
                "payload": {
                    "ucv": crate::ucan::UCAN_VERSION,
                    "iss": ucan.issuer(),
                    "aud": ucan.audience(),
                    "exp": ucan.expires_at(),
                    "nbf": ucan.not_before(),
                    "cap": {
                        "mailto:alice@email.com": {
                            "email/send": [{}]
                        }
                    },
                    "fct": {
                        "abc/challenge": { "foo": "bar" }
                    }
                },
                "signed_data": ucan.signed_data(),
                "signature": ucan.signature()
            })
        );
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_serialized_as_json_without_optionals() -> Result<()> {
        let identities = Identities::new().await;
        let ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .build()?
            .sign()
            .await?;

        let ucan_json = serde_json::to_value(ucan.clone())?;

        assert_eq!(
            ucan_json,
            serde_json::json!({
                "header": {
                    "alg": "EdDSA",
                    "typ": "JWT"
                },
                "payload": {
                    "ucv": crate::ucan::UCAN_VERSION,
                    "iss": ucan.issuer(),
                    "aud": ucan.audience(),
                    "exp": serde_json::Value::Null,
                    "cap": {}
                },
                "signed_data": ucan.signed_data(),
                "signature": ucan.signature()
            })
        );

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_implements_partial_eq() {
        let identities = Identities::new().await;
        let ucan_a = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .with_expiration(10000000)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        let ucan_b = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .with_expiration(10000000)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        let ucan_c = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .with_expiration(20000000)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        assert!(ucan_a == ucan_b);
        assert!(ucan_a != ucan_c);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_lifetime_ends_after() -> Result<()> {
        let identities = Identities::new().await;
        let forever_ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .build()?
            .sign()
            .await?;
        let early_ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .with_lifetime(2000)
            .build()?
            .sign()
            .await?;
        let later_ucan = UcanBuilder::default()
            .issued_by(&identities.alice_key)
            .for_audience(identities.bob_did.as_str())
            .with_lifetime(4000)
            .build()?
            .sign()
            .await?;

        assert_eq!(*forever_ucan.expires_at(), None);
        assert!(forever_ucan.lifetime_ends_after(&early_ucan));
        assert!(!early_ucan.lifetime_ends_after(&forever_ucan));
        assert!(later_ucan.lifetime_ends_after(&early_ucan));

        Ok(())
    }
}
