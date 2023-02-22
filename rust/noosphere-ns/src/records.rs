use crate::utils::{generate_capability, generate_fact};
use anyhow::{anyhow, Error as AnyhowError};
use cid::Cid;
use noosphere_core::{authority::SPHERE_SEMANTICS, data::Did, view::SPHERE_LIFETIME};
use noosphere_storage::{SphereDb, Storage};
use serde::{
    de::{self, Deserialize, Deserializer},
    ser::{self, Serialize, Serializer},
};
use serde_json::Value;
use std::{convert::TryFrom, str, str::FromStr};
use ucan::{builder::UcanBuilder, crypto::KeyMaterial};
use ucan::{chain::ProofChain, crypto::did::DidParser, Ucan};

/// An [NsRecord] is the internal representation of a mapping from a
/// sphere's identity (DID key) to a sphere's revision as a
/// content address ([Cid]). The record wraps a [Ucan] token,
/// providing de/serialization for transmitting in the NS network,
/// and validates data ensuring the sphere's owner authorized the publishing
/// of a new content address.
///
/// When transmitting through the distributed NS network, the record is
/// represented as the base64 encoded UCAN token.
///
/// # Ucan Semantics
///
/// An [NsRecord] is a small interface over a [Ucan] token,
/// with the following semantics:
///
/// ```json
/// {
///   // The identity (DID) of the Principal that signed the token
///   "iss": "did:key:z6MkoE19WHXJzpLqkxbGP7uXdJX38sWZNUWwyjcuCmjhPpUP",
///   // The identity (DID) of the sphere this record maps.
///   "aud": "did:key:z6MkkVfktAC5rVNRmmTjkKPapT3bAyVkYH8ZVCF1UBNUfazp",
///   // Attenuation must contain a capability with a resource "sphere:{AUD}"
///   // and action "sphere/publish".
///   "att": [{
///     "with": "sphere:did:key:z6MkkVfktAC5rVNRmmTjkKPapT3bAyVkYH8ZVCF1UBNUfazp",
///     "can": "sphere/publish"
///   }],
///   // Additional UCAN proofs needed to validate.
///   "prf": [],
///   // Facts contain a single entry with an "link" field containing
///   // the content address of a sphere revision (CID) associated with
///   // the sphere this record maps to.
///   "fct": [{
///     "link": "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72"
///   }]
/// }
/// ```
#[derive(Debug, Clone)]
pub struct NsRecord {
    /// The wrapped UCAN token describing this record.
    pub(crate) token: Ucan,
    /// The resolved sphere revision this record maps to.
    pub(crate) link: Option<Cid>,
}

impl NsRecord {
    /// Creates a new [NsRecord].
    pub fn new(token: Ucan) -> Self {
        // Cache the revision address if "fct" contains an entry matching
        // the following object without any authority validation:
        // `{ "link": "{VALID_CID}" }`
        let mut link = None;
        for ref fact in token.facts() {
            if let Value::Object(map) = fact {
                if let Some(Value::String(addr)) = map.get(&String::from("link")) {
                    if let Ok(cid) = Cid::from_str(addr) {
                        link = Some(cid);
                        break;
                    }
                }
            }
        }

        Self { token, link }
    }

    /// Creates and signs a new NsRecord from an issuer key.
    ///
    /// ```
    /// use noosphere_ns::NsRecord;
    /// use noosphere_core::{data::Did, authority::generate_ed25519_key};
    /// use noosphere_storage::{SphereDb, MemoryStorage};
    /// use ucan_key_support::ed25519::Ed25519KeyMaterial;
    /// use ucan::crypto::KeyMaterial;
    /// use cid::Cid;
    /// use tokio;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let sphere_key = generate_ed25519_key();
    ///     let sphere_id = Did::from(sphere_key.get_did().await.unwrap());
    ///     let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();
    ///     let link: Cid = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72".parse().unwrap();
    ///     let record = NsRecord::from_issuer(&sphere_key, &sphere_id, &link, None).await.unwrap();
    /// }  
    /// ```
    pub async fn from_issuer<K: KeyMaterial>(
        issuer: &K,
        sphere_id: &Did,
        link: &Cid,
        proofs: Option<&Vec<Ucan>>,
    ) -> Result<NsRecord, AnyhowError> {
        let capability = generate_capability(sphere_id);
        let fact = generate_fact(&link.to_string());

        let mut builder = UcanBuilder::default()
            .issued_by(issuer)
            .for_audience(sphere_id)
            .claiming_capability(&capability)
            .with_fact(fact);

        if let Some(proofs) = proofs {
            let mut earliest_expiry: u64 = u64::MAX;
            for token in proofs {
                earliest_expiry = *token.expires_at().min(&earliest_expiry);
                builder = builder.witnessed_by(token);
            }
            builder = builder.with_expiration(earliest_expiry);
        } else {
            builder = builder.with_lifetime(SPHERE_LIFETIME);
        }

        Ok(builder.build()?.sign().await?.into())
    }

    /// Validates the underlying [Ucan] token, ensuring that
    /// the sphere's owner authorized the publishing of a new
    /// content address. Returns an `Err` if validation fails.
    pub async fn validate<S: Storage>(
        &self,
        store: &SphereDb<S>,
        did_parser: &mut DidParser,
    ) -> Result<(), AnyhowError> {
        if self.is_expired() {
            return Err(anyhow!("Token is expired."));
        }

        let identity = self.identity();

        let desired_capability = generate_capability(identity);
        let proof = ProofChain::from_ucan(self.token.clone(), did_parser, store).await?;

        let mut has_capability = false;
        for capability_info in proof.reduce_capabilities(&SPHERE_SEMANTICS) {
            let capability = capability_info.capability;
            if capability_info.originators.contains(identity)
                && capability.enables(&desired_capability)
            {
                has_capability = true;
                break;
            }
        }

        if !has_capability {
            return Err(anyhow!("Token is not authorized to publish this sphere."));
        }

        if self.link.is_none() {
            return Err(anyhow!(
                "Missing a valid fact entry with record sphere revision. {} {:?}",
                identity,
                self.token.facts()
            ));
        }

        self.token.check_signature(did_parser).await?;
        Ok(())
    }

    /// The DID key of the sphere that this record maps.
    pub fn identity(&self) -> &str {
        self.token.audience()
    }

    /// The sphere revision address ([Cid]) that the sphere's identity maps to.
    pub fn link(&self) -> Option<&Cid> {
        self.link.as_ref()
    }

    /// Returns true if the [Ucan] token is past its expiration.
    pub fn is_expired(&self) -> bool {
        self.token.is_expired()
    }

    /// Encodes the underlying Ucan token back into a JWT string.
    pub fn try_to_string(&self) -> Result<String, AnyhowError> {
        self.token.encode()
    }
}

impl Serialize for NsRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = self.try_to_string().map_err(ser::Error::custom)?;
        serializer.serialize_str(&encoded)
    }
}

impl<'de> Deserialize<'de> for NsRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let record = NsRecord::try_from(s).map_err(de::Error::custom)?;
        Ok(record)
    }
}

/// Create a new [NsRecord] taking a [Ucan] token.
impl From<Ucan> for NsRecord {
    fn from(ucan: Ucan) -> Self {
        Self::new(ucan)
    }
}

/// Deserialize an encoded UCAN token byte vec into a [NsRecord].
impl TryFrom<Vec<u8>> for NsRecord {
    type Error = AnyhowError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        NsRecord::try_from(&bytes[..])
    }
}

/// Serialize a [NsRecord] into an encoded UCAN token byte vec.
impl TryFrom<NsRecord> for Vec<u8> {
    type Error = AnyhowError;

    fn try_from(record: NsRecord) -> Result<Self, Self::Error> {
        Vec::try_from(&record)
    }
}

/// Serialize a [NsRecord] reference into an encoded UCAN token byte vec.
impl TryFrom<&NsRecord> for Vec<u8> {
    type Error = AnyhowError;

    fn try_from(record: &NsRecord) -> Result<Vec<u8>, Self::Error> {
        Ok(Vec::from(record.token.encode()?))
    }
}

/// Deserialize an encoded UCAN token byte vec reference into a [NsRecord].
impl TryFrom<&[u8]> for NsRecord {
    type Error = AnyhowError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        NsRecord::try_from(str::from_utf8(bytes)?)
    }
}

/// Deserialize an encoded UCAN token string reference into a [NsRecord].
impl<'a> TryFrom<&'a str> for NsRecord {
    type Error = AnyhowError;

    fn try_from(ucan_token: &str) -> Result<Self, Self::Error> {
        NsRecord::from_str(ucan_token)
    }
}

/// Deserialize an encoded UCAN token string into a [NsRecord].
impl TryFrom<String> for NsRecord {
    type Error = AnyhowError;

    fn try_from(ucan_token: String) -> Result<Self, Self::Error> {
        NsRecord::from_str(ucan_token.as_str())
    }
}

/// Serialize an NsRecord into a JWT-encoded string.
impl TryFrom<NsRecord> for String {
    type Error = AnyhowError;

    fn try_from(record: NsRecord) -> Result<String, Self::Error> {
        record.try_to_string()
    }
}

/// Deserialize an encoded UCAN token string reference into a [NsRecord].
impl FromStr for NsRecord {
    type Err = AnyhowError;

    fn from_str(ucan_token: &str) -> Result<Self, Self::Err> {
        // Wait for next release of `ucan` which includes traits and
        // removes `try_from_token_string`:
        // https://github.com/ucan-wg/rs-ucan/commit/75e9afdb9da60c3d5d8c65b6704e412f0ef8189b
        Ok(NsRecord::new(Ucan::from_str(ucan_token)?))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use noosphere_core::{
        authority::{generate_ed25519_key, SUPPORTED_KEYS},
        data::Did,
    };
    use noosphere_storage::{MemoryStorage, SphereDb};
    use serde_json::json;
    use std::str::FromStr;

    use ucan::{
        builder::UcanBuilder, crypto::did::DidParser, crypto::KeyMaterial, store::UcanJwtStore,
    };

    async fn expect_failure(
        message: &str,
        store: &SphereDb<MemoryStorage>,
        did_parser: &mut DidParser,
        ucan: Ucan,
    ) {
        assert!(
            NsRecord::new(ucan)
                .validate(store, did_parser)
                .await
                .is_err(),
            "{}",
            message
        );
    }

    #[tokio::test]
    async fn test_nsrecord_self_signed() -> Result<(), AnyhowError> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let link = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let cid_link: Cid = link.parse()?;
        let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();

        let record = NsRecord::from_issuer(&sphere_key, &sphere_identity, &cid_link, None).await?;

        assert_eq!(&Did::from(record.identity()), &sphere_identity);
        assert_eq!(record.link(), Some(&cid_link));
        record
            .validate(&store, &mut DidParser::new(SUPPORTED_KEYS))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_nsrecord_delegated() -> Result<(), AnyhowError> {
        let owner_key = generate_ed25519_key();
        let owner_identity = Did::from(owner_key.get_did().await?);
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let link = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let cid_link: Cid = link.parse()?;
        let mut store = SphereDb::new(&MemoryStorage::default()).await.unwrap();

        // First verify that `owner` cannot publish for `sphere`
        // without delegation.
        let record = NsRecord::from_issuer(&owner_key, &sphere_identity, &cid_link, None).await?;

        assert_eq!(record.identity(), &sphere_identity);
        assert_eq!(record.link(), Some(&cid_link));
        if record.validate(&store, &mut did_parser).await.is_ok() {
            panic!("Owner should not have authorization to publish record")
        }

        // Delegate `sphere_key`'s publishing authority to `owner_key`
        let delegate_capability = generate_capability(&sphere_identity);
        let delegate_ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&owner_identity)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&delegate_capability)
            .build()?
            .sign()
            .await?;
        let _ = store.write_token(&delegate_ucan.encode()?).await?;

        // Attempt `owner` publishing `sphere` with the proper authorization
        let proofs = vec![delegate_ucan];
        let record =
            NsRecord::from_issuer(&owner_key, &sphere_identity, &cid_link, Some(&proofs)).await?;

        assert_eq!(record.identity(), &sphere_identity);
        assert_eq!(record.link(), Some(&cid_link));
        record.validate(&store, &mut did_parser).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_nsrecord_failures() -> Result<(), AnyhowError> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let cid_address = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();

        let sphere_capability = generate_capability(&sphere_identity);
        expect_failure(
            "fails when expect `fact` is missing",
            &store,
            &mut did_parser,
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&sphere_capability)
                .with_fact(json!({ "invalid_fact": cid_address }))
                .build()?
                .sign()
                .await?,
        )
        .await;

        let capability = generate_capability(&Did(generate_ed25519_key().get_did().await?));
        expect_failure(
            "fails when capability resource does not match sphere identity",
            &store,
            &mut did_parser,
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&capability)
                .with_fact(generate_fact(cid_address))
                .build()?
                .sign()
                .await?,
        )
        .await;

        let non_auth_key = generate_ed25519_key();
        expect_failure(
            "fails when a non-authorized key signs the record",
            &store,
            &mut did_parser,
            UcanBuilder::default()
                .issued_by(&non_auth_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&sphere_capability)
                .with_fact(generate_fact(cid_address))
                .build()?
                .sign()
                .await?,
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_nsrecord_convert() -> Result<(), AnyhowError> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let capability = generate_capability(&sphere_identity);
        let cid_address = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let fact = generate_fact(cid_address);

        let ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&sphere_identity)
            .with_lifetime(1000)
            .claiming_capability(&capability)
            .with_fact(fact)
            .build()?
            .sign()
            .await?;

        let base = NsRecord::new(ucan.clone());
        let encoded = ucan.encode()?;
        let bytes = Vec::from(encoded.clone());

        // NsRecord::serialize
        // NsRecord::deserialize
        let serialized = serde_json::to_string(&base)?;
        assert_eq!(format!("\"{}\"", encoded), serialized, "serialize()");
        let record: NsRecord = serde_json::from_str(&serialized)?;
        assert_eq!(base.identity(), record.identity(), "deserialize()");
        assert_eq!(base.link(), record.link(), "deserialize()");

        // NsRecord::try_from::<Vec<u8>>()
        let record = NsRecord::try_from(bytes.clone())?;
        assert_eq!(base.identity(), record.identity(), "try_from::<Vec<u8>>()");
        assert_eq!(base.link(), record.link(), "try_from::<Vec<u8>>()");

        // NsRecord::try_into::<Vec<u8>>()
        let rec_bytes: Vec<u8> = base.clone().try_into()?;
        assert_eq!(bytes, rec_bytes, "try_into::<Vec<u8>>()");

        // NsRecord::try_from::<&[u8]>()
        let record = NsRecord::try_from(&bytes[..])?;
        assert_eq!(base.identity(), record.identity(), "try_from::<&[u8]>()");
        assert_eq!(base.link(), record.link(), "try_from::<&[u8]>()");

        // &NsRecord::try_into::<Vec<u8>>()
        let rec_bytes: Vec<u8> = (&base).try_into()?;
        assert_eq!(bytes, rec_bytes, "&NsRecord::try_into::<Vec<u8>>()");

        // NsRecord::from::<Ucan>()
        let record = NsRecord::from(ucan);
        assert_eq!(base.identity(), record.identity(), "from::<Ucan>()");
        assert_eq!(base.link(), record.link(), "from::<Ucan>()");

        // NsRecord::try_from::<&str>()
        let record = NsRecord::try_from(encoded.as_str())?;
        assert_eq!(base.identity(), record.identity(), "try_from::<&str>()");
        assert_eq!(base.link(), record.link(), "try_from::<&str>()");

        // NsRecord::try_from::<String>()
        let record = NsRecord::try_from(encoded.clone())?;
        assert_eq!(base.identity(), record.identity(), "try_from::<String>()");
        assert_eq!(base.link(), record.link(), "try_from::<String>()");

        // NsRecord::from_str()
        let record = NsRecord::from_str(encoded.as_str())?;
        assert_eq!(base.identity(), record.identity(), "from_str()");
        assert_eq!(base.link(), record.link(), "from_str()");

        Ok(())
    }
}
