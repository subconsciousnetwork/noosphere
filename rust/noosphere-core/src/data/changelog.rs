use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct ChangelogIpld<Op> {
    pub did: Option<String>,
    pub changes: Vec<Op>,
}

impl<Op> ChangelogIpld<Op> {
    pub fn is_empty(&self) -> bool {
        self.changes.len() == 0
    }

    pub fn push(&mut self, op: Op) -> Result<()> {
        self.changes.push(op);
        Ok(())
    }

    pub fn mark(&self, did: &str) -> Self {
        ChangelogIpld {
            did: Some(did.to_string()),
            changes: Vec::new(),
        }
    }
}

impl<Op> Default for ChangelogIpld<Op> {
    fn default() -> Self {
        Self {
            did: None,
            changes: Vec::new(),
        }
    }
}
