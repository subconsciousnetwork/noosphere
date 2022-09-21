use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use std::hash::Hash;
use ucan::{
    crypto::KeyMaterial,
    store::{UcanJwtStore},
};

use noosphere_storage::{interface::BlockStore, ucan::UcanStore};
use serde::{Deserialize, Serialize};

use crate::{
    data::{CidKey, VersionedMapIpld},
    encoding::{base64_decode, base64_encode},
};

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct AuthorizationIpld {
    pub allowed: Cid,
    pub revoked: Cid,
}

impl AuthorizationIpld {
    pub async fn try_empty<S: BlockStore>(store: &mut S) -> Result<Self> {
        let allowed_ipld = AllowedIpld::try_empty(store).await?;
        let allowed = store.save::<DagCborCodec, _>(allowed_ipld).await?;
        let revoked_ipld = RevokedIpld::try_empty(store).await?;
        let revoked = store.save::<DagCborCodec, _>(revoked_ipld).await?;

        Ok(AuthorizationIpld { allowed, revoked })
    }
}

/// This delegation represents the sharing of access to resources within a
/// sphere. The name of the delegation is for display purposes only, and helps
/// the user identify the client device or application that the delegation is
/// intended for.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct DelegationIpld {
    pub name: String,
    pub jwt: Cid,
}

impl DelegationIpld {
    pub async fn try_register<S: BlockStore>(name: &str, jwt: &str, store: &S) -> Result<Self> {
        let mut store = UcanStore(store.clone());
        let cid = store.write_token(jwt).await?;

        Ok(DelegationIpld {
            name: name.to_string(),
            jwt: cid,
        })
    }
}

/// See https://github.com/ucan-wg/spec#66-revocation
/// TODO(ucan-wg/spec#112): Verify the form of this
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct RevocationIpld {
    /// Issuer's DID
    pub iss: String,
    /// JWT CID of the revoked UCAN (provisionally encoded as base64 URL-safe
    /// string)
    pub revoke: String,
    /// Issuer's signature of "REVOKE:{jwt_cid}", provisionally encoded
    /// as unpadded base64 URL-safe string
    pub challenge: String,
}

impl RevocationIpld {
    pub async fn try_revoke<K: KeyMaterial>(cid: &Cid, issuer: &K) -> Result<Self> {
        Ok(RevocationIpld {
            iss: issuer.get_did().await?,
            revoke: cid.to_string(),
            challenge: base64_encode(&issuer.sign(&Self::make_challenge_payload(cid)).await?)?,
        })
    }

    pub async fn try_verify<K: KeyMaterial + ?Sized>(&self, claimed_issuer: &K) -> Result<()> {
        let cid = Cid::try_from(self.revoke.as_str())?;
        let challenge_payload = Self::make_challenge_payload(&cid);
        let signature = base64_decode(&self.challenge)?;

        claimed_issuer
            .verify(&challenge_payload, &signature)
            .await?;

        Ok(())
    }

    fn make_challenge_payload(cid: &Cid) -> Vec<u8> {
        format!("REVOKE:{cid}").as_bytes().to_vec()
    }
}

/// The key is the CID of a UCAN JWT, and the value is the JWT itself
pub type AllowedIpld = VersionedMapIpld<CidKey, DelegationIpld>;

/// The key is the CID of the original UCAN JWT, and the value is the revocation
/// order by the UCAN issuer
pub type RevokedIpld = VersionedMapIpld<CidKey, RevocationIpld>;
