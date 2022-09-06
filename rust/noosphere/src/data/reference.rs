use cid::Cid;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct ReferenceIpld {
    pub link: Cid,
}
