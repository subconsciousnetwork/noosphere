use crate::NameSystemClient;
use anyhow;
use libp2p::{
    multiaddr::{Multiaddr, Protocol},
    PeerId,
};
use noosphere_core::authority::{SphereAction, SphereReference};
use serde_json;
use tokio::time;
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

/// A utility for [NameSystemClient] in tests.
/// Async function returns once there are at least
/// `requested_peers` peers in the network.
pub async fn wait_for_peers<T: NameSystemClient>(
    client: &T,
    requested_peers: usize,
) -> anyhow::Result<()> {
    // TODO(#101) Need to add a mechanism for non-Query based requests,
    // like sending events, or triggering a peer check on
    // new connection established. For now, we poll here.
    loop {
        let peers = client.peers().await?;
        if peers.len() >= requested_peers {
            return Ok(());
        }
        time::sleep(time::Duration::from_secs(1)).await;
    }
}

pub(crate) fn make_p2p_address(mut addr: Multiaddr, peer_id: PeerId) -> Multiaddr {
    addr.push(Protocol::P2p(peer_id.into()));
    addr
}
