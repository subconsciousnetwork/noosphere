use super::{Capability, Caveat};
use serde_json::{json, Value};
use std::fmt::{self, Debug};
use url::Url;

pub trait Scope: ToString + TryFrom<Url> + PartialEq + Clone {
    fn contains(&self, other: &Self) -> bool;
}

pub trait Ability: Ord + TryFrom<String> + ToString + Clone {}

#[derive(Clone, Eq, PartialEq)]
pub enum ResourceUri<S>
where
    S: Scope,
{
    Scoped(S),
    Unscoped,
}

impl<S> ResourceUri<S>
where
    S: Scope,
{
    pub fn contains(&self, other: &Self) -> bool {
        match self {
            ResourceUri::Unscoped => true,
            ResourceUri::Scoped(scope) => match other {
                ResourceUri::Scoped(other_scope) => scope.contains(other_scope),
                _ => false,
            },
        }
    }
}

impl<S> fmt::Display for ResourceUri<S>
where
    S: Scope,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceUri::Unscoped => write!(f, "*"),
            ResourceUri::Scoped(value) => write!(f, "{}", value.to_string()),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Resource<S>
where
    S: Scope,
{
    Resource { kind: ResourceUri<S> },
    My { kind: ResourceUri<S> },
    As { did: String, kind: ResourceUri<S> },
}

impl<S> Resource<S>
where
    S: Scope,
{
    pub fn contains(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Resource::Resource { kind: resource },
                Resource::Resource {
                    kind: other_resource,
                },
            ) => resource.contains(other_resource),
            (
                Resource::My { kind: resource },
                Resource::My {
                    kind: other_resource,
                },
            ) => resource.contains(other_resource),
            (
                Resource::As {
                    did,
                    kind: resource,
                },
                Resource::As {
                    did: other_did,
                    kind: other_resource,
                },
            ) if did == other_did => resource.contains(other_resource),
            _ => false,
        }
    }
}

impl<S> fmt::Display for Resource<S>
where
    S: Scope,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resource::Resource { kind } => write!(f, "{}", kind),
            Resource::My { kind } => write!(f, "my:{}", kind),
            Resource::As { did, kind } => write!(f, "as:{}:{}", did, kind),
        }
    }
}

pub trait CapabilitySemantics<S, A>
where
    S: Scope,
    A: Ability,
{
    fn parse_scope(&self, scope: &Url) -> Option<S> {
        S::try_from(scope.clone()).ok()
    }
    fn parse_action(&self, ability: &str) -> Option<A> {
        A::try_from(String::from(ability)).ok()
    }

    fn extract_did(&self, path: &str) -> Option<(String, String)> {
        let mut path_parts = path.split(':');

        match path_parts.next() {
            Some("did") => (),
            _ => return None,
        };

        match path_parts.next() {
            Some("key") => (),
            _ => return None,
        };

        let value = match path_parts.next() {
            Some(value) => value,
            _ => return None,
        };

        Some((format!("did:key:{value}"), path_parts.collect()))
    }

    fn parse_resource(&self, resource: &Url) -> Option<ResourceUri<S>> {
        Some(match resource.path() {
            "*" => ResourceUri::Unscoped,
            _ => ResourceUri::Scoped(self.parse_scope(resource)?),
        })
    }

    fn parse_caveat(&self, caveat: Option<&Value>) -> Value {
        if let Some(caveat) = caveat {
            caveat.to_owned()
        } else {
            json!({})
        }
    }

    /// Parse a resource and abilities string and a caveats object.
    /// The default "no caveats" (`[{}]`) is implied if `None` caveats given.
    fn parse(
        &self,
        resource: &str,
        ability: &str,
        caveat: Option<&Value>,
    ) -> Option<CapabilityView<S, A>> {
        let uri = Url::parse(resource).ok()?;

        let cap_resource = match uri.scheme() {
            "my" => Resource::My {
                kind: self.parse_resource(&uri)?,
            },
            "as" => {
                let (did, resource) = self.extract_did(uri.path())?;
                Resource::As {
                    did,
                    kind: self.parse_resource(&Url::parse(resource.as_str()).ok()?)?,
                }
            }
            _ => Resource::Resource {
                kind: self.parse_resource(&uri)?,
            },
        };

        let cap_ability = match self.parse_action(ability) {
            Some(ability) => ability,
            None => return None,
        };

        let cap_caveat = self.parse_caveat(caveat);

        Some(CapabilityView::new_with_caveat(
            cap_resource,
            cap_ability,
            cap_caveat,
        ))
    }

    fn parse_capability(&self, value: &Capability) -> Option<CapabilityView<S, A>> {
        self.parse(&value.resource, &value.ability, Some(&value.caveat))
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CapabilityView<S, A>
where
    S: Scope,
    A: Ability,
{
    pub resource: Resource<S>,
    pub ability: A,
    pub caveat: Value,
}

impl<S, A> Debug for CapabilityView<S, A>
where
    S: Scope,
    A: Ability,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capability")
            .field("resource", &self.resource.to_string())
            .field("ability", &self.ability.to_string())
            .field("caveats", &serde_json::to_string(&self.caveat))
            .finish()
    }
}

impl<S, A> CapabilityView<S, A>
where
    S: Scope,
    A: Ability,
{
    /// Creates a new [CapabilityView] semantics view over a capability
    /// without caveats.
    pub fn new(resource: Resource<S>, ability: A) -> Self {
        CapabilityView {
            resource,
            ability,
            caveat: json!({}),
        }
    }

    /// Creates a new [CapabilityView] semantics view over a capability
    /// with caveats. Note that an empty caveats array will imply NO
    /// capabilities, rendering this capability meaningless.
    pub fn new_with_caveat(resource: Resource<S>, ability: A, caveat: Value) -> Self {
        CapabilityView {
            resource,
            ability,
            caveat,
        }
    }

    pub fn enables(&self, other: &CapabilityView<S, A>) -> bool {
        match (
            Caveat::try_from(self.caveat()),
            Caveat::try_from(other.caveat()),
        ) {
            (Ok(self_caveat), Ok(other_caveat)) => {
                self.resource.contains(&other.resource)
                    && self.ability >= other.ability
                    && self_caveat.enables(&other_caveat)
            }
            _ => false,
        }
    }

    pub fn resource(&self) -> &Resource<S> {
        &self.resource
    }

    pub fn ability(&self) -> &A {
        &self.ability
    }

    pub fn caveat(&self) -> &Value {
        &self.caveat
    }
}

impl<S, A> From<&CapabilityView<S, A>> for Capability
where
    S: Scope,
    A: Ability,
{
    fn from(value: &CapabilityView<S, A>) -> Self {
        Capability::new(
            value.resource.to_string(),
            value.ability.to_string(),
            value.caveat.to_owned(),
        )
    }
}

impl<S, A> From<CapabilityView<S, A>> for Capability
where
    S: Scope,
    A: Ability,
{
    fn from(value: CapabilityView<S, A>) -> Self {
        Capability::new(
            value.resource.to_string(),
            value.ability.to_string(),
            value.caveat,
        )
    }
}
