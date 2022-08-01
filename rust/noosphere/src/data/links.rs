use cid::Cid;

use crate::data::VersionedMapIpld;

use super::{ChangelogIpld, MapOperation};

pub type LinksOperation = MapOperation<String, Cid>;
pub type LinksChangelogIpld = ChangelogIpld<LinksOperation>;
pub type LinksIpld = VersionedMapIpld<String, Cid>;
