use std::fmt::Display;

use anyhow::{anyhow, Result};
use ucan::capability::{Action, CapabilitySemantics, Scope};
use url::Url;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum GatewayAction {
    Migrate,
    Publish,
    Push,
    Fetch,
}

impl Action for GatewayAction {}

impl Display for GatewayAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                GatewayAction::Migrate => "gateway/migrate",
                GatewayAction::Publish => "gateway/publish",
                GatewayAction::Push => "gateway/push",
                GatewayAction::Fetch => "gateway/fetch",
            }
        )
    }
}

impl TryFrom<String> for GatewayAction {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(match value.as_str() {
            "gateway/migrate" => GatewayAction::Migrate,
            "gateway/publish" => GatewayAction::Publish,
            "gateway/push" => GatewayAction::Push,
            "gateway/fetch" => GatewayAction::Fetch,
            _ => return Err(anyhow!("Unrecognized action: {:?}", value)),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayIdentity {
    pub did: String,
}

impl Scope for GatewayIdentity {
    fn contains(&self, other: &Self) -> bool {
        other.did == self.did
    }
}

impl Display for GatewayIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ng:{}", self.did)
    }
}

impl TryFrom<Url> for GatewayIdentity {
    type Error = anyhow::Error;

    fn try_from(value: Url) -> Result<Self> {
        match value.scheme() {
            "ng" => Ok(GatewayIdentity {
                did: String::from(value.path()),
            }),
            _ => Err(anyhow!(
                "Could not interpret URI as a gateway reference: {:?}",
                value
            )),
        }
    }
}

pub struct GatewaySemantics {}

impl CapabilitySemantics<GatewayIdentity, GatewayAction> for GatewaySemantics {}

pub const GATEWAY_SEMANTICS: GatewaySemantics = GatewaySemantics {};
