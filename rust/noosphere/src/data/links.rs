use cid::Cid;

use crate::data::VersionedMapIpld;

pub type LinksIpld = VersionedMapIpld<String, Cid>;
