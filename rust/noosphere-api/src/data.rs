use std::fmt::Display;

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere::{
    authority::{SphereAction, SphereReference, SPHERE_SEMANTICS},
    data::Bundle,
};
use noosphere_storage::encoding::{base64_decode, base64_encode};
use serde::{Deserialize, Serialize};
use ucan::{
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    serde::Base64Encode,
    store::UcanStore,
    Ucan,
};

pub trait AsQuery {
    fn as_query(&self) -> Option<String>;
}

impl AsQuery for () {
    fn as_query(&self) -> Option<String> {
        None
    }
}

// Fetch
#[derive(Debug, Deserialize)]
pub struct FetchParameters {
    pub since: String,
    pub sphere: String,
}

impl AsQuery for FetchParameters {
    fn as_query(&self) -> Option<String> {
        Some(format!("since={}", self.since))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResponse {
    pub tip: Cid,
    pub blocks: Bundle,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutOfDateResponse {
    pub sphere: String,
    pub presumed_base: Option<Cid>,
    pub actual_tip: Cid,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingRevisionsResponse {
    pub sphere: String,
    pub presumed_base: Cid,
    pub actual_tip: Option<Cid>,
}

// Push
#[derive(Debug, Serialize, Deserialize)]
pub struct PushBody {
    pub sphere: String,
    pub base: Option<Cid>,
    pub tip: Cid,
    pub blocks: Bundle,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushResponse {
    Ok,
    OutOfDate(OutOfDateResponse),
    MissingRevisions(MissingRevisionsResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyResponse {
    pub gateway_identity: String,
    pub sphere_identity: String,
    pub signature: String,
    pub proof: String,
}

impl IdentifyResponse {
    pub async fn sign<K>(sphere_identity: &str, key: &K, proof: &Ucan) -> Result<Self>
    where
        K: KeyMaterial,
    {
        let gateway_identity = key.get_did().await?;
        let signature = base64_encode(
            &key.sign(&[gateway_identity.as_bytes(), sphere_identity.as_bytes()].concat())
                .await?,
        )?;
        Ok(IdentifyResponse {
            gateway_identity,
            sphere_identity: sphere_identity.into(),
            signature,
            proof: proof.encode()?,
        })
    }

    pub fn shares_identity_with(&self, other: &IdentifyResponse) -> bool {
        self.gateway_identity == other.gateway_identity
            && self.sphere_identity == other.sphere_identity
    }

    /// Verifies that the signature scheme on the payload. The signature is made
    /// by signing the bytes of the gateway's key DID plus the bytes of the
    /// sphere DID that the gateway claims to manage. Remember: this sphere is
    /// not the user's sphere, but rather the "counterpart" sphere created and
    /// modified by the gateway. Additionally, a proof is given that the gateway
    /// has been authorized to modify its own sphere.
    ///
    /// This verification is intended to check two things:
    ///
    ///  1. The gateway has control of the key that it represents with its DID
    ///  2. The gateway is authorized to modify the sphere it claims to manage
    pub async fn verify<S: UcanStore>(&self, did_parser: &mut DidParser, store: &S) -> Result<()> {
        let gateway_key = did_parser.parse(&self.gateway_identity)?;
        let payload_bytes = [
            self.gateway_identity.as_bytes(),
            self.sphere_identity.as_bytes(),
        ]
        .concat();
        let signature_bytes = base64_decode(&self.signature)?;

        // Verify that the signature is valid
        gateway_key.verify(&payload_bytes, &signature_bytes).await?;

        let proof = ProofChain::try_from_token_string(&self.proof, did_parser, store).await?;

        if proof.ucan().audience() != self.gateway_identity {
            return Err(anyhow!("Wrong audience!"));
        }

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: self.sphere_identity.clone(),
                }),
            },
            can: SphereAction::Push,
        };

        let capability_infos = proof.reduce_capabilities(&SPHERE_SEMANTICS);

        for capability_info in capability_infos {
            if capability_info.capability.enables(&capability)
                && capability_info.originators.contains(&self.sphere_identity)
            {
                return Ok(());
            }
        }

        Err(anyhow!("Not authorized!"))
    }
}

impl Display for IdentifyResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "((Gateway {}), (Sphere {}))",
            self.gateway_identity, self.sphere_identity
        )
    }
}
