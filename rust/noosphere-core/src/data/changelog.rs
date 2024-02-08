use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::data::{Did, VersionedMapIpld};

/// A [ChangelogIpld] records a series of changes that represent the delta of a
/// given [VersionedMapIpld] from its immediate ancestor
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct ChangelogIpld<Op> {
    /// The [Did] of the author of the change
    pub did: Option<String>,
    /// The changes that were made to the associated [VersionedMapIpld]
    pub changes: Vec<Op>,
}

impl<Op> ChangelogIpld<Op> {
    /// Returns true if the [ChangelogIpld] represents zero changes
    pub fn is_empty(&self) -> bool {
        self.changes.len() == 0
    }

    /// Adds a single change to the [ChangelogIpld]
    pub fn push(&mut self, op: Op) -> Result<()> {
        self.changes.push(op);
        Ok(())
    }

    /// Initializes a [ChangelogIpld] for the author with the given [Did]
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
