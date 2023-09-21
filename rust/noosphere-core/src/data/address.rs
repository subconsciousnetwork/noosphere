use crate::authority::{
    collect_ucan_proofs, generate_capability, SphereAbility, SPHERE_SEMANTICS, SUPPORTED_KEYS,
};
use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use serde::{de, ser, Deserialize, Serialize};
use std::fmt::Debug;
use std::{convert::TryFrom, fmt::Display, ops::Deref, str::FromStr};
use ucan::{chain::ProofChain, crypto::did::DidParser, store::UcanJwtStore, Ucan};

use super::{Did, IdentitiesIpld, Jwt, Link, MemoIpld};

#[cfg(docs)]
use crate::data::SphereIpld;

/// The name of the fact (as defined for a [Ucan]) that contains the link for a
/// [LinkRecord].
pub const LINK_RECORD_FACT_NAME: &str = "link";

/// A subdomain of a [SphereIpld] that pertains to the management and recording of
/// the petnames associated with the sphere.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct AddressBookIpld {
    /// A pointer to the [IdentitiesIpld] associated with this address book
    pub identities: Link<IdentitiesIpld>,
}

impl AddressBookIpld {
    /// Initialize an empty [AddressBookIpld], with a valid [Cid] that refers to
    /// an empty [IdentitiesIpld] in the provided storage
    pub async fn empty<S: BlockStore>(store: &mut S) -> Result<Self> {
        let identities_ipld = IdentitiesIpld::empty(store).await?;
        let identities = store.save::<DagCborCodec, _>(identities_ipld).await?.into();

        Ok(AddressBookIpld { identities })
    }
}

/// An [IdentityIpld] represents an entry in a user's pet name address book.
/// It is intended to be associated with a human readable name, and enables the
/// user to resolve the name to a DID. Eventually the DID will be resolved by
/// some mechanism to a UCAN, so this struct also records the last resolved
/// value if one has ever been resolved.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct IdentityIpld {
    /// The [Did] of a peer
    pub did: Did,
    /// An optional pointer to a known [LinkRecord] for the peer
    pub link_record: Option<Link<LinkRecord>>,
}

impl IdentityIpld {
    /// If there is a [LinkRecord] for this [IdentityIpld], attempt to retrieve
    /// it from storage
    pub async fn link_record<S: UcanJwtStore>(&self, store: &S) -> Option<LinkRecord> {
        match &self.link_record {
            Some(cid) => match store.read_token(cid).await.unwrap_or(None) {
                Some(jwt) => LinkRecord::from_str(&jwt).ok(),
                None => None,
            },
            _ => None,
        }
    }
}

/// A [LinkRecord] is a wrapper around a decoded [Jwt] ([Ucan]),
/// representing a link address as a [Cid] to a sphere.
#[derive(Clone)]
#[repr(transparent)]
pub struct LinkRecord(Ucan);

impl Debug for LinkRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LinkRecord")
            .field(
                &self
                    .0
                    .to_cid(cid::multihash::Code::Blake3_256)
                    .map_or_else(|_| String::from("<Invalid>"), |cid| cid.to_string()),
            )
            .finish()
    }
}

impl LinkRecord {
    /// Validates the [Ucan] token as a [LinkRecord], ensuring that
    /// the sphere's owner authorized the publishing of a new
    /// content address. Notably does not check the publishing timeframe
    /// permissions, as an expired token can be considered valid.
    /// Returns an `Err` if validation fails.
    pub async fn validate<S: UcanJwtStore>(&self, store: &S) -> Result<()> {
        let identity = self.to_sphere_identity();
        let token = &self.0;

        if self.get_link().is_none() {
            return Err(anyhow::anyhow!("LinkRecord missing link."));
        }

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);

        // We're interested in the validity of the proof at the time
        // of publishing.
        let now_time = if let Some(nbf) = token.not_before() {
            Some(nbf.to_owned())
        } else {
            token.expires_at().as_ref().map(|exp| exp - 1)
        };

        let proof =
            ProofChain::from_ucan(token.to_owned(), now_time, &mut did_parser, store).await?;

        {
            let desired_capability = generate_capability(&identity, SphereAbility::Publish);
            let mut has_capability = false;
            for capability_info in proof.reduce_capabilities(&SPHERE_SEMANTICS) {
                let capability = capability_info.capability;
                if capability_info.originators.contains(identity.as_str())
                    && capability.enables(&desired_capability)
                {
                    has_capability = true;
                    break;
                }
            }
            if !has_capability {
                return Err(anyhow::anyhow!("LinkRecord is not authorized."));
            }
        }

        token
            .check_signature(&mut did_parser)
            .await
            .map(|_| ())
            .map_err(|_| anyhow::anyhow!("LinkRecord has invalid signature."))
    }

    /// Returns true if the [Ucan] token is currently publishable
    /// within the bounds of its expiry/not before time.
    pub fn has_publishable_timeframe(&self) -> bool {
        !self.0.is_expired(None) && !self.0.is_too_early()
    }

    /// The DID key of the sphere that this record maps.
    pub fn to_sphere_identity(&self) -> Did {
        Did::from(self.0.audience())
    }

    /// The sphere revision address ([Link<MemoIpld>]) that the sphere's identity maps to.
    pub fn get_link(&self) -> Option<Link<MemoIpld>> {
        let facts = if let Some(facts) = self.0.facts() {
            facts
        } else {
            warn!("No facts found in the link record!");
            return None;
        };

        for (name, value) in facts.iter() {
            if name == LINK_RECORD_FACT_NAME {
                return match value.as_str() {
                    Some(link) => match Cid::try_from(link) {
                        Ok(cid) => Some(cid.into()),
                        Err(error) => {
                            warn!("Could not parse '{}' as name record link: {}", link, error);
                            None
                        }
                    },
                    None => {
                        warn!("Link record fact value must be a string.");
                        None
                    }
                };
            }
        }
        None
    }

    /// Returns a boolean indicating whether the `other` [LinkRecord]
    /// is a newer record referring to the same identity.
    /// Underlying [Ucan] expiry is used to compare. A record with
    /// `null` expiry cannot supercede or be superceded.
    pub fn superceded_by(&self, other: &LinkRecord) -> bool {
        match (self.0.expires_at(), other.0.expires_at()) {
            (Some(self_expiry), Some(other_expiry)) => {
                other_expiry > self_expiry
                    && self.to_sphere_identity() == other.to_sphere_identity()
            }
            (None, _) => false,
            (_, None) => false,
        }
    }

    /// Walk the underlying [Ucan] and collect all of the supporting proofs that
    /// verify the link publisher's authority to publish the link
    #[instrument(level = "trace", skip(self, store))]
    pub async fn collect_proofs<S>(&self, store: &S) -> Result<Vec<Ucan>>
    where
        S: UcanJwtStore,
    {
        collect_ucan_proofs(&self.0, store).await
    }
}

impl ser::Serialize for LinkRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let encoded = self.encode().map_err(ser::Error::custom)?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> de::Deserialize<'de> for LinkRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let record = LinkRecord::try_from(s).map_err(de::Error::custom)?;
        Ok(record)
    }
}

/// [LinkRecord]s compare their [Jwt] representations
/// for equality. If a record cannot be encoded as such,
/// they will not be considered equal to any other record.
impl PartialEq for LinkRecord {
    fn eq(&self, other: &Self) -> bool {
        if let Ok(encoded_a) = self.encode() {
            if let Ok(encoded_b) = other.encode() {
                return encoded_a == encoded_b;
            }
        }
        false
    }
}
impl Eq for LinkRecord {}

impl Deref for LinkRecord {
    type Target = Ucan;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LinkRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LinkRecord({}, {})",
            self.to_sphere_identity(),
            self.get_link()
                .map_or_else(|| String::from("None"), String::from)
        )
    }
}

impl TryFrom<&Jwt> for LinkRecord {
    type Error = anyhow::Error;
    fn try_from(value: &Jwt) -> Result<Self, Self::Error> {
        LinkRecord::from_str(value)
    }
}

impl TryFrom<&LinkRecord> for Jwt {
    type Error = anyhow::Error;
    fn try_from(value: &LinkRecord) -> Result<Self, Self::Error> {
        Ok(Jwt(value.encode()?))
    }
}

impl TryFrom<Jwt> for LinkRecord {
    type Error = anyhow::Error;
    fn try_from(value: Jwt) -> Result<Self, Self::Error> {
        LinkRecord::try_from(&value)
    }
}

impl TryFrom<LinkRecord> for Jwt {
    type Error = anyhow::Error;
    fn try_from(value: LinkRecord) -> Result<Self, Self::Error> {
        Jwt::try_from(&value)
    }
}

impl From<&Ucan> for LinkRecord {
    fn from(value: &Ucan) -> Self {
        LinkRecord::from(value.to_owned())
    }
}

impl From<&LinkRecord> for Ucan {
    fn from(value: &LinkRecord) -> Self {
        value.0.clone()
    }
}

impl From<Ucan> for LinkRecord {
    fn from(value: Ucan) -> Self {
        LinkRecord(value)
    }
}

impl From<LinkRecord> for Ucan {
    fn from(value: LinkRecord) -> Self {
        value.0
    }
}

impl TryFrom<&[u8]> for LinkRecord {
    type Error = anyhow::Error;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        LinkRecord::try_from(value.to_vec())
    }
}

impl TryFrom<Vec<u8>> for LinkRecord {
    type Error = anyhow::Error;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        LinkRecord::from_str(&String::from_utf8(value)?)
    }
}

impl TryFrom<LinkRecord> for Vec<u8> {
    type Error = anyhow::Error;
    fn try_from(value: LinkRecord) -> Result<Self, Self::Error> {
        Ok(value.encode()?.into_bytes())
    }
}

impl FromStr for LinkRecord {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Ucan::from_str(value)?.into())
    }
}

impl TryFrom<String> for LinkRecord {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Ucan::from_str(&value)?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        authority::generate_ed25519_key,
        data::Did,
        tracing::initialize_tracing,
        view::{Sphere, SPHERE_LIFETIME},
    };
    use noosphere_storage::{MemoryStorage, SphereDb, UcanStore};
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    pub async fn from_issuer<K: KeyMaterial>(
        issuer: &K,
        sphere_id: &Did,
        link: &Cid,
        proofs: Option<&Vec<Ucan>>,
    ) -> Result<LinkRecord, anyhow::Error> {
        let capability = generate_capability(sphere_id, SphereAbility::Publish);

        let mut builder = UcanBuilder::default()
            .issued_by(issuer)
            .for_audience(sphere_id)
            .claiming_capability(&capability)
            .with_fact(LINK_RECORD_FACT_NAME, link.to_string());

        if let Some(proofs) = proofs {
            let mut earliest_expiry: u64 = u64::MAX;
            for token in proofs {
                if let Some(exp) = token.expires_at() {
                    earliest_expiry = *exp.min(&earliest_expiry);
                    builder = builder.witnessed_by(token, None);
                }
            }
            builder = builder.with_expiration(earliest_expiry);
        } else {
            builder = builder.with_lifetime(SPHERE_LIFETIME);
        }

        Ok(builder.build()?.sign().await?.into())
    }

    async fn expect_failure(message: &str, store: &SphereDb<MemoryStorage>, record: LinkRecord) {
        assert!(record.validate(store).await.is_err(), "{}", message);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_self_signed_link_record() -> Result<()> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let link = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i";
        let cid_link: Link<MemoIpld> = link.parse()?;
        let store = SphereDb::new(MemoryStorage::default()).await.unwrap();

        let record = from_issuer(&sphere_key, &sphere_identity, &cid_link, None).await?;

        assert_eq!(&record.to_sphere_identity(), &sphere_identity);
        assert_eq!(LinkRecord::get_link(&record), Some(cid_link));
        LinkRecord::validate(&record, &store).await?;
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_delegated_link_record() -> Result<()> {
        let owner_key = generate_ed25519_key();
        let owner_identity = Did::from(owner_key.get_did().await?);
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let link = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i";
        let cid_link: Cid = link.parse()?;
        let mut store = SphereDb::new(MemoryStorage::default()).await.unwrap();

        // First verify that `owner` cannot publish for `sphere`
        // without delegation.
        let record = from_issuer(&owner_key, &sphere_identity, &cid_link, None).await?;

        assert_eq!(record.to_sphere_identity(), sphere_identity);
        assert_eq!(record.get_link(), Some(cid_link.into()));
        if LinkRecord::validate(&record, &store).await.is_ok() {
            panic!("Owner should not have authorization to publish record")
        }

        // Delegate `sphere_key`'s publishing authority to `owner_key`
        let delegate_ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&owner_identity)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .build()?
            .sign()
            .await?;
        let _ = store.write_token(&delegate_ucan.encode()?).await?;

        // Attempt `owner` publishing `sphere` with the proper authorization.
        let proofs = vec![delegate_ucan.clone()];
        let record = from_issuer(&owner_key, &sphere_identity, &cid_link, Some(&proofs)).await?;

        assert_eq!(record.to_sphere_identity(), sphere_identity);
        assert_eq!(record.get_link(), Some(cid_link.into()));
        assert!(LinkRecord::has_publishable_timeframe(&record));
        LinkRecord::validate(&record, &store).await?;

        // Now test a similar record that has an expired capability.
        // It must still be valid.
        let expired: LinkRecord = UcanBuilder::default()
            .issued_by(&owner_key)
            .for_audience(&sphere_identity)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .with_fact(LINK_RECORD_FACT_NAME, cid_link.to_string())
            .witnessed_by(&delegate_ucan, None)
            .with_expiration(ucan::time::now() - 1234)
            .build()?
            .sign()
            .await?
            .into();
        assert_eq!(expired.to_sphere_identity(), sphere_identity);
        assert_eq!(expired.get_link(), Some(cid_link.into()));
        assert!(!expired.has_publishable_timeframe());
        LinkRecord::validate(&record, &store).await?;
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_link_record_failures() -> Result<()> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let cid_address = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i";
        let store = SphereDb::new(MemoryStorage::default()).await.unwrap();

        expect_failure(
            "fails when expect `fact` is missing",
            &store,
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&generate_capability(
                    sphere_identity.as_str(),
                    SphereAbility::Publish,
                ))
                .with_fact("invalid-fact", cid_address.to_owned())
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await;

        let capability = generate_capability(
            &Did(generate_ed25519_key().get_did().await?),
            SphereAbility::Publish,
        );
        expect_failure(
            "fails when capability resource does not match sphere identity",
            &store,
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&capability)
                .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await;

        let non_auth_key = generate_ed25519_key();
        expect_failure(
            "fails when a non-authorized key signs the record",
            &store,
            UcanBuilder::default()
                .issued_by(&non_auth_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&generate_capability(
                    &sphere_identity,
                    SphereAbility::Publish,
                ))
                .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await;

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_link_record_convert() -> Result<()> {
        let sphere_key = generate_ed25519_key();
        let identity = Did::from(sphere_key.get_did().await?);
        let capability = generate_capability(&identity, SphereAbility::Publish);
        let cid_address = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i";
        let link = Cid::from_str(cid_address)?;
        let maybe_link = Some(link.into());

        let ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&identity)
            .with_lifetime(1000)
            .claiming_capability(&capability)
            .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
            .build()?
            .sign()
            .await?;

        let encoded = ucan.encode()?;
        let base = LinkRecord::from(ucan.clone());

        // from_str, String
        {
            let record: LinkRecord = encoded.parse()?;
            assert_eq!(
                record.to_sphere_identity(),
                identity,
                "LinkRecord::from_str()"
            );
            assert_eq!(record.get_link(), maybe_link, "LinkRecord::from_str()");
            let record: LinkRecord = encoded.clone().try_into()?;
            assert_eq!(
                record.to_sphere_identity(),
                identity,
                "LinkRecord::try_from(String)"
            );
            assert_eq!(
                record.get_link(),
                maybe_link,
                "LinkRecord::try_from(String)"
            );
        }

        // Ucan convert
        {
            let from_ucan_ref = LinkRecord::from(&ucan);
            assert_eq!(
                base.to_sphere_identity(),
                identity,
                "LinkRecord::from(Ucan)"
            );
            assert_eq!(base.get_link(), maybe_link, "LinkRecord::from(Ucan)");
            assert_eq!(
                from_ucan_ref.to_sphere_identity(),
                identity,
                "LinkRecord::from(&Ucan)"
            );
            assert_eq!(
                from_ucan_ref.get_link(),
                maybe_link,
                "LinkRecord::from(&Ucan)"
            );
            assert_eq!(
                Ucan::from(base.clone()).encode()?,
                encoded,
                "Ucan::from(LinkRecord)"
            );
            assert_eq!(
                Ucan::from(&base).encode()?,
                encoded,
                "Ucan::from(&LinkRecord)"
            );
        };

        // Vec<u8> convert
        {
            let bytes = Vec::from(encoded.clone());
            let record = LinkRecord::try_from(bytes.clone())?;
            assert_eq!(
                record.to_sphere_identity(),
                identity,
                "LinkRecord::try_from(Vec<u8>)"
            );
            assert_eq!(
                record.get_link(),
                maybe_link,
                "LinkRecord::try_from(Vec<u8>)"
            );

            let record = LinkRecord::try_from(bytes.as_slice())?;
            assert_eq!(
                record.to_sphere_identity(),
                identity,
                "LinkRecord::try_from(&[u8])"
            );
            assert_eq!(record.get_link(), maybe_link, "LinkRecord::try_from(&[u8])");

            let bytes_from_record: Vec<u8> = record.try_into()?;
            assert_eq!(bytes_from_record, bytes, "LinkRecord::try_into(Vec<u8>>)");
        };

        // LinkRecord::serialize
        // LinkRecord::deserialize
        {
            let serialized = serde_json::to_string(&base)?;
            assert_eq!(serialized, format!("\"{}\"", encoded), "serialize()");
            let record: LinkRecord = serde_json::from_str(&serialized)?;
            assert_eq!(record.to_sphere_identity(), identity, "deserialize()");
            assert_eq!(record.get_link(), maybe_link, "deserialize()");
        }

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_collect_related_proofs_from_storage() -> Result<()> {
        initialize_tracing(None);
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let delegatee_key = generate_ed25519_key();
        let delegatee_did = delegatee_key.get_did().await?;

        let mut db = SphereDb::new(MemoryStorage::default()).await?;
        let mut ucan_store = UcanStore(db.clone());

        let (sphere, proof, _) = Sphere::generate(&owner_did, &mut db).await?;
        let ucan = proof.as_ucan(&db).await?;

        let sphere_identity = sphere.get_identity().await?;

        let delegated_ucan = UcanBuilder::default()
            .issued_by(&owner_key)
            .for_audience(&delegatee_did)
            .witnessed_by(&ucan, None)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .with_lifetime(120)
            .build()?
            .sign()
            .await?;

        let link_record_ucan = UcanBuilder::default()
            .issued_by(&delegatee_key)
            .for_audience(&sphere_identity)
            .witnessed_by(&delegated_ucan, None)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .with_lifetime(120)
            .with_fact(LINK_RECORD_FACT_NAME, sphere.cid().to_string())
            .build()?
            .sign()
            .await?;

        let link_record = LinkRecord::from(link_record_ucan.clone());

        ucan_store.write_token(&ucan.encode()?).await?;
        ucan_store.write_token(&delegated_ucan.encode()?).await?;
        ucan_store.write_token(&link_record.encode()?).await?;

        let proofs = link_record.collect_proofs(&ucan_store).await?;

        assert_eq!(proofs.len(), 3);
        assert_eq!(vec![link_record_ucan, delegated_ucan, ucan], proofs);

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_superceded_by() -> Result<()> {
        let sphere_key = generate_ed25519_key();
        let identity = Did::from(sphere_key.get_did().await?);
        let capability = generate_capability(&identity, SphereAbility::Publish);
        let cid_address = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i";
        let other_key = generate_ed25519_key();
        let other_identity = Did::from(other_key.get_did().await?);

        let earlier = LinkRecord::from(
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&identity)
                .with_lifetime(1000)
                .claiming_capability(&capability)
                .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
                .build()?
                .sign()
                .await?,
        );

        let later = LinkRecord::from(
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&identity)
                .with_lifetime(2000)
                .claiming_capability(&capability)
                .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
                .build()?
                .sign()
                .await?,
        );

        let no_expiry = LinkRecord::from(
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&identity)
                .claiming_capability(&capability)
                .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
                .build()?
                .sign()
                .await?,
        );

        let other_identity = LinkRecord::from(
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&other_identity)
                .claiming_capability(&generate_capability(
                    &other_identity,
                    SphereAbility::Publish,
                ))
                .with_fact(LINK_RECORD_FACT_NAME, cid_address.to_owned())
                .build()?
                .sign()
                .await?,
        );

        assert!(earlier.superceded_by(&later));
        assert!(!later.superceded_by(&earlier));
        assert!(!earlier.superceded_by(&no_expiry));
        assert!(!earlier.superceded_by(&other_identity));
        assert!(!no_expiry.superceded_by(&later));
        assert!(!other_identity.superceded_by(&later));
        Ok(())
    }
}
