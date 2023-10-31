use std::fmt::Display;

use crate::api::data::{empty_string_as_none, AsQuery};
use crate::{
    authority::{generate_capability, SphereAbility, SPHERE_SEMANTICS},
    data::{Bundle, Did, Jwt, Link, MemoIpld},
    error::NoosphereError,
};
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_storage::{base64_decode, base64_encode};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ucan::{
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    store::UcanStore,
    Ucan,
};

/// The query parameters expected for the "replicate" API route.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplicateParameters {
    /// This is the last revision of the content that is being fetched that is
    /// already fully available to the caller of the API.
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub since: Option<Link<MemoIpld>>,

    /// If true, all content in the sphere's content space as of the associated
    /// version will be replicated along with the sphere itself. If this field
    /// is used without a specific `since`, then the replication request is
    /// assumed to be for the whole of a single version of a sphere (and not its
    /// history).
    #[serde(default)]
    pub include_content: bool,
}

impl AsQuery for ReplicateParameters {
    fn as_query(&self) -> Result<Option<String>> {
        Ok(self.since.as_ref().map(|since| format!("since={since}")))
    }
}

/// Allowed types in the route fragment for selecting a replication target.
#[derive(Clone)]
pub enum ReplicationMode {
    /// Replicate by [Cid]; the specific version will be replicated
    Cid(Cid),
    /// Replicate by [Did]; gives up authority to the gateway to decide what
    /// version ought to be replicated
    Did(Did),
}

impl From<Cid> for ReplicationMode {
    fn from(value: Cid) -> Self {
        ReplicationMode::Cid(value)
    }
}

impl From<Did> for ReplicationMode {
    fn from(value: Did) -> Self {
        ReplicationMode::Did(value)
    }
}

impl Display for ReplicationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplicationMode::Cid(cid) => Display::fmt(cid, f),
            ReplicationMode::Did(did) => Display::fmt(did, f),
        }
    }
}

/// The query parameters expected for the "fetch" API route
#[derive(Debug, Serialize, Deserialize)]
pub struct FetchParameters {
    /// This is the last revision of the "counterpart" sphere that is managed
    /// by the API host that the client is fetching from
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub since: Option<Link<MemoIpld>>,
}

impl AsQuery for FetchParameters {
    fn as_query(&self) -> Result<Option<String>> {
        Ok(self.since.as_ref().map(|since| format!("since={since}")))
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
    },
    /// There are no new revisions since the revision specified in the initial
    /// fetch request
    UpToDate,
}

/// The body payload expected by the "push" API route
#[derive(Debug, Serialize, Deserialize)]
pub struct PushBody {
    /// The DID of the local sphere whose revisions are being pushed
    pub sphere: Did,
    /// The base revision represented by the payload being pushed; if the
    /// entire history is being pushed, then this should be None
    pub local_base: Option<Link<MemoIpld>>,
    /// The tip of the history represented by the payload being pushed
    pub local_tip: Link<MemoIpld>,
    /// The last received tip of the counterpart sphere
    pub counterpart_tip: Option<Link<MemoIpld>>,
    /// A bundle of all the blocks needed to hydrate the revisions from the
    /// base to the tip of history as represented by this payload
    pub blocks: Bundle,
    /// An optional name record to publish to the Noosphere Name System
    pub name_record: Option<Jwt>,
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
        new_tip: Link<MemoIpld>,
        /// The blocks needed to hydrate the revisions of the "counterpart"
        /// sphere history to the tip represented in this response
        blocks: Bundle,
    },
    /// The history was already known by the API host, so no changes were made
    NoChange,
}

/// Error types for typical "push" API failure conditions
#[derive(Error, Debug)]
pub enum PushError {
    #[allow(missing_docs)]
    #[error("Pushed history conflicts with canonical history")]
    Conflict,
    #[allow(missing_docs)]
    #[error("Missing some implied history")]
    MissingHistory,
    #[allow(missing_docs)]
    #[error("Replica is up to date")]
    UpToDate,
    #[allow(missing_docs)]
    #[error("Internal error")]
    Internal(anyhow::Error),
}

impl From<NoosphereError> for PushError {
    fn from(error: NoosphereError) -> Self {
        error.into()
    }
}

impl From<anyhow::Error> for PushError {
    fn from(value: anyhow::Error) -> Self {
        PushError::Internal(value)
    }
}

impl From<PushError> for StatusCode {
    fn from(error: PushError) -> Self {
        match error {
            PushError::Conflict => StatusCode::CONFLICT,
            PushError::MissingHistory => StatusCode::UNPROCESSABLE_ENTITY,
            PushError::UpToDate => StatusCode::BAD_REQUEST,
            PushError::Internal(error) => {
                error!("Internal: {:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
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
    /// Create and sign an [IdentifyResponse] with the provided credential
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

    /// Compare one [IdentifyResponse] with another to verify that they refer to
    /// the same gateway
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

        let proof = ProofChain::try_from_token_string(&self.proof, None, did_parser, store).await?;

        if proof.ucan().audience() != self.gateway_identity.as_str() {
            return Err(anyhow!("Wrong audience!"));
        }

        let capability = generate_capability(&self.sphere_identity, SphereAbility::Push);
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
