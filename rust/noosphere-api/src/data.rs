use cid::Cid;
use noosphere::data::Bundle;
use serde::{Deserialize, Serialize};

pub trait AsQuery {
    fn as_query(&self) -> Option<String>;
}

impl AsQuery for () {
    fn as_query(&self) -> Option<String> {
        None
    }
}

// Fetch
#[derive(Debug, Deserialize)]
pub struct FetchParameters {
    pub since: String,
    pub sphere: String,
}

impl AsQuery for FetchParameters {
    fn as_query(&self) -> Option<String> {
        Some(format!("since={}", self.since))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResponse {
    pub tip: Cid,
    pub blocks: Bundle,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutOfDateResponse {
    pub sphere: String,
    pub presumed_base: Option<Cid>,
    pub actual_tip: Cid,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingRevisionsResponse {
    pub sphere: String,
    pub presumed_base: Cid,
    pub actual_tip: Option<Cid>,
}

// Push
#[derive(Debug, Serialize, Deserialize)]
pub struct PushBody {
    pub sphere: String,
    pub base: Option<Cid>,
    pub tip: Cid,
    pub blocks: Bundle,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushResponse {
    Ok,
    OutOfDate(OutOfDateResponse),
    MissingRevisions(MissingRevisionsResponse),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdentifyResponse {
    pub identity: String,
}
