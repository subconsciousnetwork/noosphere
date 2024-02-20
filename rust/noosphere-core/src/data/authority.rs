use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_ucan::{crypto::KeyMaterial, store::UcanJwtStore, Ucan};
use std::{hash::Hash, str::FromStr};

use noosphere_storage::{base64_decode, base64_encode, BlockStore, UcanStore};
use serde::{Deserialize, Serialize};

use super::{DelegationsIpld, Link, RevocationsIpld};

#[cfg(docs)]
use crate::data::SphereIpld;

/// A subdomain of a [SphereIpld] that pertains to the delegated authority to
/// access a sphere, as well as the revocations of that authority.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct AuthorityIpld {
    /// A pointer to the [DelegationsIpld] for this [AuthorityIpld], embodying
    /// all authorizations for keys that operate on this sphere
    pub delegations: Link<DelegationsIpld>,
    /// A pointer to the [RevocationsIpld] for this [AuthorityIpld], embodying
    /// revocations for any otherwise valid authorizations for keys issued by this
    /// sphere in the past.
    pub revocations: Link<RevocationsIpld>,
}

impl AuthorityIpld {
    /// Initialize an empty [AuthorityIpld], with valid [Cid]s referring to
    /// empty [DelegationsIpld] and [RevocationsIpld] that are persisted in the
    /// provided storage
    pub async fn empty<S: BlockStore>(store: &mut S) -> Result<Self> {
        let delegations_ipld = DelegationsIpld::empty(store).await?;
        let delegations = store
            .save::<DagCborCodec, _>(delegations_ipld)
            .await?
            .into();
        let revocations_ipld = RevocationsIpld::empty(store).await?;
        let revocations = store
            .save::<DagCborCodec, _>(revocations_ipld)
            .await?
            .into();

        Ok(AuthorityIpld {
            delegations,
            revocations,
        })
    }
}

#[cfg(doc)]
use crate::data::Jwt;

/// This delegation represents the sharing of access to resources within a
/// sphere. The name of the delegation is for display purposes only, and helps
/// the user identify the client device or application that the delegation is
/// intended for.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Serialize, Deserialize, Hash)]
pub struct DelegationIpld {
    /// The human-readable name of the delegation
    pub name: String,
    /// A pointer to the [Jwt] created for this [DelegationIpld]
    pub jwt: Cid,
}

impl DelegationIpld {
    /// Stores a [Ucan] that delegates authority to a key, and initializes a
    /// [DelegationIpld] for it that is appropriate for storing in a sphere
    pub async fn register<S: BlockStore>(name: &str, jwt: &str, store: &S) -> Result<Self> {
        let mut store = UcanStore(store.clone());
        let cid = store.write_token(jwt).await?;

        Ok(DelegationIpld {
            name: name.to_string(),
            jwt: cid,
        })
    }

    /// Resolve a [Ucan] from storage via the pointer to a [Jwt] in this
    /// [DelegationIpld]
    pub async fn resolve_ucan<S: BlockStore>(&self, store: &S) -> Result<Ucan> {
        let store = UcanStore(store.clone());
        let jwt = store.require_token(&self.jwt).await?;

        Ucan::from_str(&jwt)
    }
}

/// See <https://github.com/ucan-wg/spec#66-revocation>
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
    /// Revoke a delegation by the [Cid] of its associated [Jwt], using a key credential
    /// of an authorizing ancestor of the original delegation
    pub async fn revoke<K: KeyMaterial>(cid: &Cid, issuer: &K) -> Result<Self> {
        Ok(RevocationIpld {
            iss: issuer.get_did().await?,
            revoke: cid.to_string(),
            challenge: base64_encode(&issuer.sign(&Self::make_challenge_payload(cid)).await?)?,
        })
    }

    /// Verify that the [RevocationIpld] is valid compared to the public key of
    /// the issuer
    pub async fn verify<K: KeyMaterial + ?Sized>(&self, claimed_issuer: &K) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use noosphere_storage::{MemoryStore, UcanStore};
    use noosphere_ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};

    use crate::authority::generate_ed25519_key;

    use super::{DelegationIpld, RevocationIpld};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_stores_a_registerd_jwt() {
        let store = MemoryStore::default();
        let key = generate_ed25519_key();

        let ucan_jwt = UcanBuilder::default()
            .issued_by(&key)
            .for_audience(&key.get_did().await.unwrap())
            .with_lifetime(128)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        let delegation = DelegationIpld::register("foobar", &ucan_jwt, &store)
            .await
            .unwrap();

        assert_eq!(
            UcanStore(store).read_token(&delegation.jwt).await.unwrap(),
            Some(ucan_jwt)
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_verify_that_a_key_issued_a_revocation() {
        let store = MemoryStore::default();
        let key = generate_ed25519_key();
        let other_key = generate_ed25519_key();

        let ucan_jwt = UcanBuilder::default()
            .issued_by(&key)
            .for_audience(&key.get_did().await.unwrap())
            .with_lifetime(128)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap()
            .encode()
            .unwrap();

        let delegation = DelegationIpld::register("foobar", &ucan_jwt, &store)
            .await
            .unwrap();

        let revocation = RevocationIpld::revoke(&delegation.jwt, &key).await.unwrap();

        assert!(revocation.verify(&key).await.is_ok());
        assert!(revocation.verify(&other_key).await.is_err());
    }
}
