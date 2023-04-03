use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt::Display, ops::Deref};
use ucan::{store::UcanJwtStore, Ucan};

use super::{Did, IdentitiesIpld, Jwt, Link};

#[cfg(docs)]
use crate::data::SphereIpld;

/// A subdomain of a [SphereIpld] that pertains to the management and recording of
/// the petnames associated with the sphere.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct AddressBookIpld {
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
    pub did: Did,
    pub link_record: Option<Link<LinkRecord>>,
}

impl IdentityIpld {
    /// If there is a [LinkRecord] for this [IdentityIpld], attempt to retrieve
    /// it from storage
    pub async fn link_record<S: UcanJwtStore>(&self, store: &S) -> Option<LinkRecord> {
        match &self.link_record {
            Some(cid) => store
                .read_token(cid)
                .await
                .unwrap_or(None)
                .map(|jwt| LinkRecord(jwt.into())),
            _ => None,
        }
    }
}

/// A [LinkRecord] is a newtype that represents a JWT that ought to contain a
/// [Cid] reference to a sphere
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
#[serde(from = "Jwt", into = "Jwt")]
#[repr(transparent)]
pub struct LinkRecord(Jwt);

impl LinkRecord {
    /// Parse the wrapped [Jwt] as a [Ucan] and looks for the referenced pointer
    /// to some data in IPFS (via a [Cid] in the `fct` field).
    pub async fn dereference(&self) -> Option<Cid> {
        let token = &self.0;
        let ucan = match Ucan::try_from(token.to_string()) {
            Ok(ucan) => ucan,
            _ => return None,
        };
        let facts = ucan.facts();

        for fact in facts {
            match fact.as_object() {
                Some(fields) => match fields.get("link") {
                    Some(cid_string) => {
                        match Cid::try_from(cid_string.as_str().unwrap_or_default()) {
                            Ok(cid) => return Some(cid),
                            Err(error) => {
                                warn!(
                                    "Could not parse '{}' as name record link: {}",
                                    cid_string, error
                                );
                                continue;
                            }
                        }
                    }
                    None => {
                        warn!("No 'link' field in fact, skipping...");
                        continue;
                    }
                },
                None => {
                    warn!("Fact is not an object, skipping...");
                    continue;
                }
            }
        }

        warn!("No facts contained a link!");

        None
    }
}

impl Deref for LinkRecord {
    type Target = Jwt;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LinkRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Jwt> for LinkRecord {
    fn from(value: Jwt) -> Self {
        LinkRecord(value)
    }
}

impl Into<Jwt> for LinkRecord {
    fn into(self) -> Jwt {
        self.0
    }
}
