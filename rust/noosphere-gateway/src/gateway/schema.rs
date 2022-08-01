use cid::Cid;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedSphere {
    pub root: Cid,
}
