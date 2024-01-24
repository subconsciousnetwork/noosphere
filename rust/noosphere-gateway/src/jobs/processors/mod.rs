//! Functions called by [GatewayJobProcessor] to process [GatewayJob]s.

#[cfg(doc)]
use crate::jobs::{GatewayJob, GatewayJobProcessor};

mod cleanup;
mod name_system;
mod syndication;

pub use cleanup::*;
pub use name_system::*;
pub use syndication::*;
