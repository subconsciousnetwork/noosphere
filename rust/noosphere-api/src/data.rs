use std::{fmt::Display, str::FromStr};

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    authority::{SphereAction, SphereReference, SPHERE_SEMANTICS},
    data::{Bundle, Did},
};
use noosphere_storage::encoding::{base64_decode, base64_encode};
use serde::{Deserialize, Deserializer, Serialize};
use ucan::{
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    store::UcanStore,
    Ucan,
};

pub trait AsQuery {
    fn as_query(&self) -> Result<Option<String>>;
}

impl AsQuery for () {
    fn as_query(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

// NOTE: Adapted from https://github.com/tokio-rs/axum/blob/7caa4a3a47a31c211d301f3afbc518ea2c07b4de/examples/query-params-with-empty-strings/src/main.rs#L42-L54
/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s)
            .map_err(serde::de::Error::custom)
            .map(Some),
    }
}

/// The parameters expected for the "fetch" API route
#[derive(Debug, Serialize, Deserialize)]
pub struct FetchParameters {
    /// This is the last revision of the "counterpart" sphere that is managed
    /// by the API host that the client is fetching from
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub since: Option<Cid>,
}

impl AsQuery for FetchParameters {
    fn as_query(&self) -> Result<Option<String>> {
        Ok(self.since.as_ref().map(|since| format!("since={}", since)))
    }
}

/// The possible responses from the "fetch" API route
#[derive(Debug, Serialize, Deserialize)]
pub enum FetchResponse {
    /// There are new revisions to the local and "counterpart" spheres to sync
    /// with local history
    NewChanges {
        /// The tip of the "counterpart" sphere that is managed by the API host
        /// that the client is fetching from
        tip: Cid,
        /// All the new blocks of the "counterpart" sphere as well as the new
        /// blocks of the local sphere that correspond to remote changes from
        /// other clients
        blocks: Bundle,
    },
    /// There are no new revisions since the revision specified in the initial
    /// fetch request
    UpToDate,
}

/// The body payload expected by the "push" API route
#[derive(Debug, Serialize, Deserialize)]
pub struct PushBody {
    /// The DID of the local sphere whose revisions are being pushed
    pub sphere: String,
    /// The base revision represented by the payload being pushed; if the
    /// entire history is being pushed, then this should be None
    pub base: Option<Cid>,
    /// The tip of the history represented by the payload being pushed
    pub tip: Cid,
    /// A bundle of all the blocks needed to hydrate the revisions from the
    /// base to the tip of history as represented by this payload
    pub blocks: Bundle,
}

/// The possible responses from the "push" API route
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushResponse {
    /// The new history was accepted
    Accepted {
        /// This is the new tip of the "counterpart" sphere after accepting
        /// the latest history from the local sphere. This is guaranteed to be
        /// at least one revision ahead of the latest revision being tracked
        /// by the client (because it points to the newly received tip of the
        /// local sphere's history)
        new_tip: Cid,
        /// The blocks needed to hydrate the revisions of the "counterpart"
        /// sphere history to the tip represented in this response
        blocks: Bundle,
    },
    /// The history was already known by the API host, so no changes were made
    NoChange,
}

/// The response from the "identify" API route; this is a signed response that
/// allows the client to verify the authority of the API host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyResponse {
    /// The DID of the API host
    pub gateway_identity: Did,
    /// The DID of the "counterpart" sphere
    pub sphere_identity: Did,
    /// The signature of the API host over this payload, as base64-encoded bytes
    pub signature: String,
    /// The proof that the API host was authorized by the "counterpart" sphere
    /// in the form of a UCAN JWT
    pub proof: String,
}

impl IdentifyResponse {
    pub async fn sign<K>(sphere_identity: &str, key: &K, proof: &Ucan) -> Result<Self>
    where
        K: KeyMaterial,
    {
        let gateway_identity = Did(key.get_did().await?);
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

        if proof.ucan().audience() != self.gateway_identity.as_str() {
            return Err(anyhow!("Wrong audience!"));
        }

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: self.sphere_identity.to_string(),
                }),
            },
            can: SphereAction::Push,
        };

        let capability_infos = proof.reduce_capabilities(&SPHERE_SEMANTICS);

        for capability_info in capability_infos {
            if capability_info.capability.enables(&capability)
                && capability_info
                    .originators
                    .contains(self.sphere_identity.as_str())
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
