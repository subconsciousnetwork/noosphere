use std::convert::TryFrom;

use cid::Cid;
use serde::{Deserialize, Serialize};
use ucan::Ucan;

use super::{Did, Jwt};

/// An [AddressIpld] represents an entry in a user's pet name address book.
/// It is intended to be associated with a human readable name, and enables the
/// user to resolve the name to a DID. Eventually the DID will be resolved by
/// some mechanism to a UCAN, so this struct also records the last resolved
/// value if one has ever been resolved.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct AddressIpld {
    pub identity: Did,
    pub last_known_record: Option<Jwt>,
}

impl AddressIpld {
    /// If a last known record is available, parses it as a [Ucan] and
    /// looks for the referenced pointer to some data in IPFS (via a [Cid]
    /// in the `fct` field).
    pub async fn dereference(&self) -> Option<Cid> {
        match &self.last_known_record {
            Some(token) => {
                let ucan = match Ucan::try_from(token.to_string()) {
                    Ok(ucan) => ucan,
                    _ => return None,
                };
                let facts = ucan.facts();

                for fact in facts {
                    match fact.as_object() {
                        Some(fields) => match fields.get("link") {
                            Some(cid_string) => match Cid::try_from(cid_string.to_string()) {
                                Ok(cid) => return Some(cid),
                                _ => continue,
                            },
                            None => continue,
                        },
                        None => continue,
                    }
                }

                None
            }
            None => None,
        }
    }
}
