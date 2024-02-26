use crate::{DhtClient, NameResolver};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use libp2p::{
    multiaddr::{Multiaddr, Protocol},
    PeerId,
};
use noosphere_core::data::{Did, Link, MemoIpld};
use tokio::time;

/// Additional utilites for [NameResolver] implementations.
#[async_trait]
pub trait NameResolverPoller: NameResolver {
    /// Polls a [NameResolver] until `identity` resolves to [LinkRecord]
    /// containing `link`, or `timeout` seconds have elapsed (defaults to 5 seconds).
    ///
    /// In tests, used after syncing a Noosphere client, ensuring a record
    /// has been published to the name system before another gateway attempts
    /// to resolve its address book on push; otherwise, we must rely on
    /// periodic resolution of address books sometime in the future.
    async fn wait_for_record(
        &self,
        identity: &Did,
        link: &Link<MemoIpld>,
        timeout: Option<u64>,
    ) -> Result<()> {
        let timeout = timeout.unwrap_or(5);
        let mut attempts = timeout;
        loop {
            if attempts < 1 {
                return Err(anyhow!(
                    "Name record not published after {} seconds.",
                    timeout
                ));
            }
            if let Some(record) = self.resolve(identity).await? {
                if let Some(record_link) = record.get_link() {
                    if &record_link == link {
                        return Ok(());
                    }
                }
            }
            attempts -= 1;
            time::sleep(time::Duration::from_secs(1)).await;
        }
    }
}
impl<T> NameResolverPoller for T where T: NameResolver {}

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
