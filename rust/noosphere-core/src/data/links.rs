// use cid::Cid;

use crate::data::VersionedMapIpld;

use super::MemoIpld;

pub type LinksIpld = VersionedMapIpld<String, MemoIpld>;
