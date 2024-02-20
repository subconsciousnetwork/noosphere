use crate::DhtClient;
use libp2p::{
    multiaddr::{Multiaddr, Protocol},
    PeerId,
};
use tokio::time;

/// A utility for [NameSystemClient] in tests.
/// Async function returns once there are at least
/// `requested_peers` peers in the network.
pub async fn wait_for_peers<T: DhtClient>(
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
    addr.push(Protocol::P2p(peer_id));
    addr
}
