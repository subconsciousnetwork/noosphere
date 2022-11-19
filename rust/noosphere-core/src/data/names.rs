use crate::data::VersionedMapIpld;

use super::AddressIpld;

pub type NamesIpld = VersionedMapIpld<String, AddressIpld>;
