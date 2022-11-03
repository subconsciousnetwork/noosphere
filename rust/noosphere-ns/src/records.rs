use crate::utils::generate_capability;
use anyhow::{anyhow, Error};
use cid::Cid;
use noosphere_core::authority::SPHERE_SEMANTICS;
use noosphere_storage::{db::SphereDb, interface::Store};
use serde_json::Value;
use std::{convert::TryFrom, str, str::FromStr};
use ucan::{chain::ProofChain, crypto::did::DidParser, Ucan};

/// An [NSRecord] is the internal representation of a mapping from a
/// sphere's identity (DID key) to its content address ([cid::Cid]),
/// providing validation and de/serialization functionality for
/// transmitting in the distributed NS network.
/// The record wraps a [ucan::Ucan] token, containing validation
/// data ensuring the sphere's owner authorized the publishing
/// of a new content address.
///
/// When transmitting through the distributed NS network, the record is
/// represented as the base64 encoded Ucan token.
///
/// # Ucan Semantics
///
/// An [NSRecord] is a small interface over a [ucan::Ucan] token,
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
///   // Additional Ucan proofs needed to validate.
///   "prf": [],
///   // Facts contain a single entry with an "address" field containing
///   // the content address (CID) that the record's sphere's identity maps to.
///   "fct": [{
///     "address": "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72"
///   }]
/// }
/// ```
#[derive(Debug, Clone)]
pub struct NSRecord {
    /// The wrapped Ucan token describing this record.
    pub(crate) token: Ucan,
    /// The resolved content address this record maps to.
    pub(crate) address: Option<Cid>,
}

impl NSRecord {
    /// Creates a new [NSRecord].
    pub fn new(token: Ucan) -> Self {
        // Cache the address if "fct" contains an entry matching
        // the following object without any authority validation:
        // `{ "address": "{VALID_CID}" }`
        let mut address = None;
        for ref fact in token.facts() {
            if let Value::Object(map) = fact {
                if let Some(Value::String(addr)) = map.get(&String::from("address")) {
                    if let Ok(cid) = Cid::from_str(addr) {
                        address = Some(cid);
                        break;
                    }
                }
            }
        }

        Self { token, address }
    }

    /// Validates the underlying [ucan::Ucan] token, ensuring that
    /// the sphere's owner authorized the publishing of a new
    /// content address. Returns an `Err` if validation fails.
    pub async fn validate<S: Store>(
        &mut self,
        store: &SphereDb<S>,
        did_parser: &mut DidParser,
    ) -> Result<(), Error> {
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

        if self.address.is_none() {
            return Err(anyhow!(
                "Missing a valid fact entry with record address. {} {:?}",
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

    /// The sphere revision ([cid::Cid]) that the sphere's identity maps to.
    pub fn address(&self) -> Option<&Cid> {
        self.address.as_ref()
    }

    /// Returns true if the UCAN token is past its expiration.
    pub fn is_expired(&self) -> bool {
        self.token.is_expired()
    }
}

impl From<Ucan> for NSRecord {
    fn from(ucan: Ucan) -> Self {
        Self::new(ucan)
    }
}

/// Deserialize an encoded UCAN token byte vec into a [NSRecord].
impl TryFrom<Vec<u8>> for NSRecord {
    type Error = anyhow::Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        NSRecord::try_from(&bytes[..])
    }
}

/// Serialize a [NSRecord] into an encoded UCAN token byte vec.
impl TryFrom<NSRecord> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(record: NSRecord) -> Result<Self, Self::Error> {
        Vec::try_from(&record)
    }
}

/// Deserialize an encoded UCAN token byte vec reference into a [NSRecord].
impl TryFrom<&[u8]> for NSRecord {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        NSRecord::try_from(str::from_utf8(bytes)?)
    }
}

/// Serialize a [NSRecord] reference into an encoded UCAN token byte vec.
impl TryFrom<&NSRecord> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(record: &NSRecord) -> Result<Self, Self::Error> {
        Ok(Vec::from(record.token.encode()?))
    }
}

/// Deserialize an encoded UCAN token string reference into a [NSRecord].
impl<'a> TryFrom<&'a str> for NSRecord {
    type Error = anyhow::Error;

    fn try_from(ucan_token: &str) -> Result<Self, Self::Error> {
        NSRecord::from_str(ucan_token)
    }
}

/// Deserialize an encoded UCAN token string into a [NSRecord].
impl TryFrom<String> for NSRecord {
    type Error = anyhow::Error;

    fn try_from(ucan_token: String) -> Result<Self, Self::Error> {
        NSRecord::from_str(ucan_token.as_str())
    }
}

/// Deserialize an encoded UCAN token string reference into a [NSRecord].
impl FromStr for NSRecord {
    type Err = anyhow::Error;

    fn from_str(ucan_token: &str) -> Result<Self, Self::Err> {
        // Wait for next release of `ucan` which includes traits and
        // removes `try_from_token_string`:
        // https://github.com/ucan-wg/rs-ucan/commit/75e9afdb9da60c3d5d8c65b6704e412f0ef8189b
        Ok(NSRecord::new(Ucan::try_from_token_string(ucan_token)?))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use noosphere_core::authority::{generate_ed25519_key, SUPPORTED_KEYS};
    use noosphere_storage::{
        db::SphereDb,
        memory::{MemoryStorageProvider, MemoryStore},
    };
    use serde_json::json;
    use std::str::FromStr;
    
    use ucan::{builder::UcanBuilder, crypto::did::DidParser, crypto::KeyMaterial};

    async fn expect_failure(
        message: &str,
        store: &SphereDb<MemoryStore>,
        did_parser: &mut DidParser,
        ucan: Ucan,
    ) {
        assert!(
            NSRecord::new(ucan)
                .validate(store, did_parser)
                .await
                .is_err(),
            "{}",
            message
        );
    }

    #[tokio::test]
    async fn test_nsrecord_self_signed() -> Result<(), Error> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = sphere_key.get_did().await?;
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let capability = generate_capability(&sphere_identity);
        let cid_address = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let fact = json!({ "address": cid_address });
        let store = SphereDb::new(&MemoryStorageProvider::default())
            .await
            .unwrap();

        let mut record = NSRecord::new(
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&capability)
                .with_fact(fact)
                .build()?
                .sign()
                .await?,
        );

        assert_eq!(record.identity(), &sphere_identity);
        assert_eq!(record.address(), Some(&Cid::from_str(cid_address).unwrap()));
        record.validate(&store, &mut did_parser).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_nsrecord_delegated() -> Result<(), Error> {
        // TODO
        Ok(())
    }

    #[tokio::test]
    async fn test_nsrecord_failures() -> Result<(), Error> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = sphere_key.get_did().await?;
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let cid_address = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let store = SphereDb::new(&MemoryStorageProvider::default())
            .await
            .unwrap();

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

        let capability = generate_capability(&generate_ed25519_key().get_did().await?);
        expect_failure(
            "fails when capability resource does not match sphere identity",
            &store,
            &mut did_parser,
            UcanBuilder::default()
                .issued_by(&sphere_key)
                .for_audience(&sphere_identity)
                .with_lifetime(1000)
                .claiming_capability(&capability)
                .with_fact(json!({ "address": cid_address }))
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
                .with_fact(json!({ "address": cid_address }))
                .build()?
                .sign()
                .await?,
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_nsrecord_convert() -> Result<(), Error> {
        let sphere_key = generate_ed25519_key();
        let sphere_identity = sphere_key.get_did().await?;
        let capability = generate_capability(&sphere_identity);
        let cid_address = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72";
        let fact = json!({ "address": cid_address });

        let ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&sphere_identity)
            .with_lifetime(1000)
            .claiming_capability(&capability)
            .with_fact(fact)
            .build()?
            .sign()
            .await?;

        let base = NSRecord::new(ucan.clone());
        let encoded = ucan.encode()?;
        let bytes = Vec::from(encoded.clone());

        // NSRecord::try_from::<Vec<u8>>()
        let record = NSRecord::try_from(bytes.clone())?;
        assert_eq!(base.identity(), record.identity(), "try_from::<Vec<u8>>()");
        assert_eq!(base.address(), record.address(), "try_from::<Vec<u8>>()");

        // NSRecord::try_into::<Vec<u8>>()
        let rec_bytes: Vec<u8> = base.clone().try_into()?;
        assert_eq!(bytes, rec_bytes, "try_into::<Vec<u8>>()");

        // NSRecord::try_from::<&[u8]>()
        let record = NSRecord::try_from(&bytes[..])?;
        assert_eq!(base.identity(), record.identity(), "try_from::<&[u8]>()");
        assert_eq!(base.address(), record.address(), "try_from::<&[u8]>()");

        // &NSRecord::try_into::<Vec<u8>>()
        let rec_bytes: Vec<u8> = (&base).try_into()?;
        assert_eq!(bytes, rec_bytes, "&NSRecord::try_into::<Vec<u8>>()");

        // NSRecord::from::<Ucan>()
        let record = NSRecord::from(ucan);
        assert_eq!(base.identity(), record.identity(), "from::<Ucan>()");
        assert_eq!(base.address(), record.address(), "from::<Ucan>()");

        // NSRecord::try_from::<&str>()
        let record = NSRecord::try_from(encoded.as_str())?;
        assert_eq!(base.identity(), record.identity(), "try_from::<&str>()");
        assert_eq!(base.address(), record.address(), "try_from::<&str>()");

        // NSRecord::try_from::<String>()
        let record = NSRecord::try_from(encoded.clone())?;
        assert_eq!(base.identity(), record.identity(), "try_from::<String>()");
        assert_eq!(base.address(), record.address(), "try_from::<String>()");

        // NSRecord::from_str()
        let record = NSRecord::from_str(encoded.as_str())?;
        assert_eq!(base.identity(), record.identity(), "from_str()");
        assert_eq!(base.address(), record.address(), "from_str()");

        Ok(())
    }
}
