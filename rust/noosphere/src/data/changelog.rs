use anyhow::{anyhow, Result};
use crdts::VClock;
use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct ChangelogIpld<Op> {
    pub base: VClock<String>,
    pub tip: VClock<String>,
    pub did: Option<String>,
    pub changes: Vec<Op>,
}

impl<Op> ChangelogIpld<Op> {
    pub fn push(&mut self, op: Op) -> Result<()> {
        let did = self
            .did
            .as_ref()
            .ok_or_else(|| anyhow!("No DID set as change author"))?;

        self.tip.inc(did.clone());
        self.changes.push(op);
        Ok(())
    }

    pub fn mark(&self, did: &str) -> Self {
        ChangelogIpld {
            base: self.tip.clone(),
            tip: self.tip.clone(),
            did: Some(did.to_string()),
            changes: Vec::new(),
        }
    }
}

impl<Op> Default for ChangelogIpld<Op> {
    fn default() -> Self {
        Self {
            base: Default::default(),
            tip: Default::default(),
            did: None,
            changes: Vec::new(),
        }
    }
}
