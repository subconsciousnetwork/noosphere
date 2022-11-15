use serde::{Deserialize, Serialize};

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
