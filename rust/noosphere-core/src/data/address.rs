use std::convert::TryFrom;

use cid::Cid;
use serde::{Deserialize, Serialize};
use ucan::{store::UcanJwtStore, Ucan};

use super::{Did, Jwt};

/// An [AddressIpld] represents an entry in a user's pet name address book.
/// It is intended to be associated with a human readable name, and enables the
/// user to resolve the name to a DID. Eventually the DID will be resolved by
/// some mechanism to a UCAN, so this struct also records the last resolved
/// value if one has ever been resolved.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct AddressIpld {
    pub identity: Did,
    pub last_known_record: Option<Cid>,
}

impl AddressIpld {
    /// If there is a last known record, attempt to retrieve it from storage as
    /// a [Jwt] proof token
    pub async fn get_proof<S: UcanJwtStore>(&self, store: &S) -> Option<Jwt> {
        match &self.last_known_record {
            Some(cid) => store
                .read_token(cid)
                .await
                .unwrap_or(None)
                .map(|jwt| jwt.into()),
            _ => None,
        }
    }

    /// If a last known record is available, parses it as a [Ucan] and
    /// looks for the referenced pointer to some data in IPFS (via a [Cid]
    /// in the `fct` field).
    pub async fn dereference<S: UcanJwtStore>(&self, store: &S) -> Option<Cid> {
        match &self.last_known_record {
            Some(cid) => {
                let token = match store.require_token(cid).await {
                    Ok(jwt) => jwt,
                    Err(error) => {
                        error!("Failed to look up token for {}: {}", cid, error);
                        return None;
                    }
                };
                let ucan = match Ucan::try_from(token) {
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
            None => {
                warn!("No record recorded for this address!");

                None
            }
        }
    }
}
