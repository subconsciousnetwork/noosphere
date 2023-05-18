use anyhow::{anyhow, Result};
use ucan::capability::{Action, Capability, CapabilitySemantics, Resource, Scope, With};
use url::Url;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum SphereAction {
    /// May read information about a sphere from a counterpart
    Fetch,
    /// May push an updated sphere lineage to a counterpart
    Push,
    /// May publish a canonical revision to the Noosphere Name System
    Publish,
    /// May transfer the authority delegated by a sphere to another key
    Authorize,
}

impl Action for SphereAction {}

impl ToString for SphereAction {
    fn to_string(&self) -> String {
        match self {
            SphereAction::Authorize => "sphere/authorize",
            SphereAction::Publish => "sphere/publish",
            SphereAction::Push => "sphere/push",
            SphereAction::Fetch => "sphere/fetch",
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
            "sphere/push" => SphereAction::Push,
            "sphere/fetch" => SphereAction::Fetch,
            _ => return Err(anyhow!("Unrecognized action: {:?}", value)),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Generates a [Capability] struct representing permissions in a [LinkRecord].
///
/// ```
/// use noosphere_core::{authority::{generate_capability, SphereAction, SphereReference}};
/// use ucan::capability::{Capability, Resource, With};
///
/// let identity = "did:key:z6MkoE19WHXJzpLqkxbGP7uXdJX38sWZNUWwyjcuCmjhPpUP";
/// let expected_capability = Capability {
///     with: With::Resource {
///         kind: Resource::Scoped(SphereReference {
///            did: identity.to_owned(),
///         }),
///     },
///     can: SphereAction::Publish,
/// };
/// assert_eq!(generate_capability(&identity, SphereAction::Publish), expected_capability);
/// ```
pub fn generate_capability(
    identity: &str,
    action: SphereAction,
) -> Capability<SphereReference, SphereAction> {
    Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: identity.to_owned(),
            }),
        },
        can: action,
    }
}
