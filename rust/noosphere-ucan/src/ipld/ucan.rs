use crate::{
    capability::Capabilities,
    crypto::JwtSignatureAlgorithm,
    ipld::{Principle, Signature},
    serde::Base64Encode,
    ucan::{FactsMap, Ucan, UcanHeader, UcanPayload, UCAN_VERSION},
};
use cid::Cid;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UcanIpld {
    pub v: String,

    pub iss: Principle,
    pub aud: Principle,
    pub s: Signature,

    pub cap: Capabilities,
    pub prf: Option<Vec<Cid>>,
    pub exp: Option<u64>,
    pub fct: Option<FactsMap>,

    pub nnc: Option<String>,
    pub nbf: Option<u64>,
}

impl TryFrom<&Ucan> for UcanIpld {
    type Error = anyhow::Error;

    fn try_from(ucan: &Ucan) -> Result<Self, Self::Error> {
        let prf = if let Some(proofs) = ucan.proofs() {
            let mut prf = Vec::new();
            for cid_string in proofs {
                prf.push(Cid::try_from(cid_string.as_str())?);
            }
            if prf.is_empty() {
                None
            } else {
                Some(prf)
            }
        } else {
            None
        };

        Ok(UcanIpld {
            v: ucan.version().to_string(),
            iss: Principle::from_str(ucan.issuer())?,
            aud: Principle::from_str(ucan.audience())?,
            s: Signature::try_from((
                JwtSignatureAlgorithm::from_str(ucan.algorithm())?,
                ucan.signature(),
            ))?,
            cap: ucan.capabilities().clone(),
            prf,
            exp: *ucan.expires_at(),
            fct: ucan.facts().clone(),
            nnc: ucan.nonce().as_ref().cloned(),
            nbf: *ucan.not_before(),
        })
    }
}

impl TryFrom<&UcanIpld> for Ucan {
    type Error = anyhow::Error;

    fn try_from(value: &UcanIpld) -> Result<Self, Self::Error> {
        let (algorithm, signature) = value.s.decode()?;

        let header = UcanHeader {
            alg: algorithm.to_string(),
            typ: "JWT".into(),
        };

        let payload = UcanPayload {
            ucv: UCAN_VERSION.into(),
            iss: value.iss.to_string(),
            aud: value.aud.to_string(),
            exp: value.exp,
            nbf: value.nbf,
            nnc: value.nnc.clone(),
            cap: value.cap.clone(),
            fct: value.fct.clone(),
            prf: value
                .prf
                .clone()
                .map(|prf| prf.iter().map(|cid| cid.to_string()).collect()),
        };

        let signed_data = format!(
            "{}.{}",
            header.jwt_base64_encode()?,
            payload.jwt_base64_encode()?
        )
        .as_bytes()
        .to_vec();

        Ok(Ucan::new(header, payload, signed_data, signature))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use serde_json::json;

    use crate::{
        tests::{
            fixtures::Identities,
            helpers::{dag_cbor_roundtrip, scaffold_ucan_builder},
        },
        Ucan,
    };

    use super::UcanIpld;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_produces_canonical_jwt_despite_json_ambiguity() {
        let identities = Identities::new().await;
        let canon_builder = scaffold_ucan_builder(&identities).await.unwrap();
        let other_builder = scaffold_ucan_builder(&identities).await.unwrap();

        let canon_jwt = canon_builder
            .with_fact(
                "abc/challenge",
                json!({
                    "baz": true,
                    "foo": "bar"
                }),
            )
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        let other_jwt = other_builder
            .with_fact(
                "abc/challenge",
                json!({
                    "foo": "bar",
                    "baz": true
                }),
            )
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        assert_eq!(canon_jwt, other_jwt);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_stays_canonical_when_converting_between_jwt_and_ipld() {
        let identities = Identities::new().await;
        let builder = scaffold_ucan_builder(&identities).await.unwrap();

        let jwt = builder
            .with_fact(
                "abc/challenge",
                json!({
                    "baz": true,
                    "foo": "bar"
                }),
            )
            .with_nonce()
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        let ucan = Ucan::try_from(jwt.as_str()).unwrap();
        let ucan_ipld = UcanIpld::try_from(&ucan).unwrap();

        let decoded_ucan_ipld = dag_cbor_roundtrip(&ucan_ipld).unwrap();

        let decoded_ucan = Ucan::try_from(&decoded_ucan_ipld).unwrap();

        let decoded_jwt = decoded_ucan.encode().unwrap();

        assert_eq!(jwt, decoded_jwt);
    }
}
