use noosphere_core::authority::{SphereAction, SphereReference};
use serde_json;
use ucan::capability::{Capability, Resource, With};

#[cfg(doc)]
use cid::Cid;

/// Generates a [Capability] struct representing permission to
/// publish a sphere.
///
/// ```
/// use noosphere_ns::utils::generate_capability;
/// use noosphere_core::{authority::{SphereAction, SphereReference}};
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
/// assert_eq!(generate_capability(&identity), expected_capability);
/// ```
pub fn generate_capability(identity: &str) -> Capability<SphereReference, SphereAction> {
    Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: identity.to_owned(),
            }),
        },
        can: SphereAction::Publish,
    }
}

/// Generates a UCAN `"fct"` struct for the NS network, representing
/// the resolved sphere's revision as a [Cid].
///
/// ```
/// use noosphere_ns::utils::generate_fact;
/// use noosphere_storage::derive_cid;
/// use libipld_cbor::DagCborCodec;
/// use serde_json::json;
///  
/// let address = "bafy2bzaced25m65oooyocdin7uyehm7u6eak3iauyxbxxvoos6atwe7vvmv46";
/// assert_eq!(generate_fact(address), json!({ "link": address }));
/// ```
pub fn generate_fact(address: &str) -> serde_json::Value {
    serde_json::json!({ "link": address })
}
