use anyhow::{anyhow, Result};
use ucan::capability::{Action, CapabilitySemantics, Scope};
use url::Url;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum SphereAction {
    Authorize,
    Publish,
    Sign,
}

impl Action for SphereAction {}

impl ToString for SphereAction {
    fn to_string(&self) -> String {
        match self {
            SphereAction::Authorize => "sphere/authorize",
            SphereAction::Publish => "sphere/publish",
            SphereAction::Sign => "sphere/sign",
        }
        .into()
    }
}

impl TryFrom<String> for SphereAction {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(match value.as_str() {
            "sphere/authorize" => SphereAction::Authorize,
            "sphere/publish" => SphereAction::Publish,
            "sphere/sign" => SphereAction::Sign,
            _ => return Err(anyhow!("Unrecognized action: {:?}", value)),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SphereReference {
    pub did: String,
}

impl Scope for SphereReference {
    fn contains(&self, other: &Self) -> bool {
        other.did == self.did
    }
}

impl ToString for SphereReference {
    fn to_string(&self) -> String {
        format!("sphere:{}", self.did)
    }
}

impl TryFrom<Url> for SphereReference {
    type Error = anyhow::Error;

    fn try_from(value: Url) -> Result<Self> {
        match value.scheme() {
            "sphere" => Ok(SphereReference {
                did: String::from(value.path()),
            }),
            _ => Err(anyhow!(
                "Could not interpret URI as a sphere reference: {:?}",
                value
            )),
        }
    }
}

pub struct SphereSemantics {}

impl CapabilitySemantics<SphereReference, SphereAction> for SphereSemantics {}

pub const SPHERE_SEMANTICS: SphereSemantics = SphereSemantics {};
