use crate::{
    capability::Capabilities,
    crypto::did::DidParser,
    serde::{Base64Encode, DagJson},
    time::now,
};
use anyhow::{anyhow, Result};
use base64::Engine;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use libipld_core::{codec::Codec, raw::RawCodec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, str::FromStr};

pub const UCAN_VERSION: &str = "0.10.0-canary";

pub type FactsMap = BTreeMap<String, Value>;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct UcanHeader {
    pub alg: String,
    pub typ: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct UcanPayload {
    pub ucv: String,
    pub iss: String,
    pub aud: String,
    pub exp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nnc: Option<String>,
    pub cap: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fct: Option<FactsMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prf: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Ucan {
    header: UcanHeader,
    payload: UcanPayload,
    signed_data: Vec<u8>,
    signature: Vec<u8>,
}

impl Ucan {
    pub fn new(
        header: UcanHeader,
        payload: UcanPayload,
        signed_data: Vec<u8>,
        signature: Vec<u8>,
    ) -> Self {
        Ucan {
            signed_data,
            header,
            payload,
            signature,
        }
    }

    /// Validate the UCAN's signature and timestamps
    pub async fn validate<'a>(
        &self,
        now_time: Option<u64>,
        did_parser: &mut DidParser,
    ) -> Result<()> {
        if self.is_expired(now_time) {
            return Err(anyhow!("Expired"));
        }

        if self.is_too_early() {
            return Err(anyhow!("Not active yet (too early)"));
        }

        self.check_signature(did_parser).await
    }

    /// Validate that the signed data was signed by the stated issuer
    pub async fn check_signature<'a>(&self, did_parser: &mut DidParser) -> Result<()> {
        let key = did_parser.parse(&self.payload.iss)?;
        key.verify(&self.signed_data, &self.signature).await
    }

    /// Produce a base64-encoded serialization of the UCAN suitable for
    /// transferring in a header field
    pub fn encode(&self) -> Result<String> {
        let header = self.header.jwt_base64_encode()?;
        let payload = self.payload.jwt_base64_encode()?;
        let signature =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.signature.as_slice());

        Ok(format!("{header}.{payload}.{signature}"))
    }

    /// Returns true if the UCAN has past its expiration date
    pub fn is_expired(&self, now_time: Option<u64>) -> bool {
        if let Some(exp) = self.payload.exp {
            exp < now_time.unwrap_or_else(now)
        } else {
            false
        }
    }

    /// Raw bytes of signed data for this UCAN
    pub fn signed_data(&self) -> &[u8] {
        &self.signed_data
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// Returns true if the not-before ("nbf") time is still in the future
    pub fn is_too_early(&self) -> bool {
        match self.payload.nbf {
            Some(nbf) => nbf > now(),
            None => false,
        }
    }

    /// Returns true if this UCAN's lifetime begins no later than the other
    /// Note that if a UCAN specifies an NBF but the other does not, the
    /// other has an unbounded start time and this function will return
    /// false.
    pub fn lifetime_begins_before(&self, other: &Ucan) -> bool {
        match (self.payload.nbf, other.payload.nbf) {
            (Some(nbf), Some(other_nbf)) => nbf <= other_nbf,
            (Some(_), None) => false,
            _ => true,
        }
    }

    /// Returns true if this UCAN expires no earlier than the other
    pub fn lifetime_ends_after(&self, other: &Ucan) -> bool {
        match (self.payload.exp, other.payload.exp) {
            (Some(exp), Some(other_exp)) => exp >= other_exp,
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    /// Returns true if this UCAN's lifetime fully encompasses the other
    pub fn lifetime_encompasses(&self, other: &Ucan) -> bool {
        self.lifetime_begins_before(other) && self.lifetime_ends_after(other)
    }

    pub fn algorithm(&self) -> &str {
        &self.header.alg
    }

    pub fn issuer(&self) -> &str {
        &self.payload.iss
    }

    pub fn audience(&self) -> &str {
        &self.payload.aud
    }

    pub fn proofs(&self) -> &Option<Vec<String>> {
        &self.payload.prf
    }

    pub fn expires_at(&self) -> &Option<u64> {
        &self.payload.exp
    }

    pub fn not_before(&self) -> &Option<u64> {
        &self.payload.nbf
    }

    pub fn nonce(&self) -> &Option<String> {
        &self.payload.nnc
    }

    #[deprecated(since = "0.4.0", note = "use `capabilities()`")]
    pub fn attenuation(&self) -> &Capabilities {
        self.capabilities()
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.payload.cap
    }

    pub fn facts(&self) -> &Option<FactsMap> {
        &self.payload.fct
    }

    pub fn version(&self) -> &str {
        &self.payload.ucv
    }

    pub fn to_cid(&self, hasher: Code) -> Result<Cid> {
        let codec = RawCodec;
        let token = self.encode()?;
        let encoded = codec.encode(token.as_bytes())?;
        Ok(Cid::new_v1(codec.into(), hasher.digest(&encoded)))
    }
}

/// Deserialize an encoded UCAN token string reference into a UCAN
impl<'a> TryFrom<&'a str> for Ucan {
    type Error = anyhow::Error;

    fn try_from(ucan_token: &str) -> Result<Self, Self::Error> {
        Ucan::from_str(ucan_token)
    }
}

/// Deserialize an encoded UCAN token string into a UCAN
impl TryFrom<String> for Ucan {
    type Error = anyhow::Error;

    fn try_from(ucan_token: String) -> Result<Self, Self::Error> {
        Ucan::from_str(ucan_token.as_str())
    }
}

/// Deserialize an encoded UCAN token string reference into a UCAN
impl FromStr for Ucan {
    type Err = anyhow::Error;

    fn from_str(ucan_token: &str) -> Result<Self, Self::Err> {
        // better to create multiple iterators than collect, or clone.
        let signed_data = ucan_token
            .split('.')
            .take(2)
            .map(String::from)
            .reduce(|l, r| format!("{l}.{r}"))
            .ok_or_else(|| anyhow!("Could not parse signed data from token string"))?;

        let mut parts = ucan_token.split('.').map(|str| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(str)
                .map_err(|error| anyhow!(error))
        });

        let header = parts
            .next()
            .ok_or_else(|| anyhow!("Missing UCAN header in token part"))?
            .map(|decoded| UcanHeader::from_dag_json(&decoded))
            .map_err(|e| e.context("Could not decode UCAN header base64"))?
            .map_err(|e| e.context("Could not parse UCAN header JSON"))?;

        let payload = parts
            .next()
            .ok_or_else(|| anyhow!("Missing UCAN payload in token part"))?
            .map(|decoded| UcanPayload::from_dag_json(&decoded))
            .map_err(|e| e.context("Could not decode UCAN payload base64"))?
            .map_err(|e| e.context("Could not parse UCAN payload JSON"))?;

        let signature = parts
            .next()
            .ok_or_else(|| anyhow!("Missing UCAN signature in token part"))?
            .map_err(|e| e.context("Could not parse UCAN signature base64"))?;

        Ok(Ucan::new(
            header,
            payload,
            signed_data.as_bytes().into(),
            signature,
        ))
    }
}
