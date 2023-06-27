use anyhow::{anyhow, Result};
use serde_json::json;
use ucan::capability::{
    Ability, CapabilitySemantics, CapabilityView, Resource, ResourceUri, Scope,
};
use url::Url;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum SphereAbility {
    /// May read information about a sphere from a counterpart
    Fetch,
    /// May push an updated sphere lineage to a counterpart
    Push,
    /// May publish a canonical revision to the Noosphere Name System
    Publish,
    /// May transfer the authority delegated by a sphere to another key
    Authorize,
}

impl Ability for SphereAbility {}

impl ToString for SphereAbility {
    fn to_string(&self) -> String {
        match self {
            SphereAbility::Authorize => "sphere/authorize",
            SphereAbility::Publish => "sphere/publish",
            SphereAbility::Push => "sphere/push",
            SphereAbility::Fetch => "sphere/fetch",
        }
        .into()
    }
}

impl TryFrom<String> for SphereAbility {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(match value.as_str() {
            "sphere/authorize" => SphereAbility::Authorize,
            "sphere/publish" => SphereAbility::Publish,
            "sphere/push" => SphereAbility::Push,
            "sphere/fetch" => SphereAbility::Fetch,
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

impl CapabilitySemantics<SphereReference, SphereAbility> for SphereSemantics {}

pub const SPHERE_SEMANTICS: SphereSemantics = SphereSemantics {};

/// Generates a [Capability] struct representing permissions in a [LinkRecord].
///
/// ```
/// use noosphere_core::{authority::{generate_capability, SphereAbility, SphereReference}};
/// use ucan::capability::{CapabilityView, ResourceUri, Resource};
/// use serde_json::json;
///
/// let identity = "did:key:z6MkoE19WHXJzpLqkxbGP7uXdJX38sWZNUWwyjcuCmjhPpUP";
/// let expected_capability = CapabilityView {
///     resource: Resource::Resource {
///         kind: ResourceUri::Scoped(SphereReference {
///            did: identity.to_owned(),
///         }),
///     },
///     ability: SphereAbility::Publish,
///     caveat: json!({}),
/// };
/// assert_eq!(generate_capability(&identity, SphereAbility::Publish), expected_capability);
/// ```
pub fn generate_capability(
    identity: &str,
    ability: SphereAbility,
) -> CapabilityView<SphereReference, SphereAbility> {
    CapabilityView {
        resource: Resource::Resource {
            kind: ResourceUri::Scoped(SphereReference {
                did: identity.to_owned(),
            }),
        },
        ability,
        caveat: json!({}),
    }
}
