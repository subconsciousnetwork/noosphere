use crate::{
    dht::{NetworkInfo, Peer},
    PeerId,
};
use anyhow::Result;
use async_trait::async_trait;
use libp2p::Multiaddr;
use noosphere_core::data::{Did, LinkRecord};

#[cfg(doc)]
use crate::server::HttpClient;
#[cfg(doc)]
use crate::NameSystem;

#[async_trait]
pub trait DhtClient: Send + Sync {
    /* Diagnostic APIs */

    /// Returns current network information for this node.
    async fn network_info(&self) -> Result<NetworkInfo>;

    /// Returns the [PeerId] based on the provided key.
    fn peer_id(&self) -> &PeerId;

    /* Peering APIs */

    /// Returns current network information for this node.
    async fn peers(&self) -> Result<Vec<Peer>>;

    /// Adds peers to connect to. Unless bootstrapping a network, at least one
    /// peer is needed.
    async fn add_peers(&self, peers: Vec<Multiaddr>) -> Result<()>;

    /* Listening APIs */

    /// Starts listening for connections on provided address.
    async fn listen(&self, listening_address: Multiaddr) -> Result<Multiaddr>;

    /// Stops listening for incoming connections.
    async fn stop_listening(&self) -> Result<()>;

    /// Returns the listening addresses of this node.
    async fn address(&self) -> Result<Option<Multiaddr>>;

    /* Record APIs */

    /// Propagates the corresponding managed sphere's [LinkRecord] on nearby peers
    /// in the DHT network.
    async fn put_record(&self, record: LinkRecord, quorum: usize) -> Result<()>;

    /// Returns an [LinkRecord] for the provided identity if found.
    async fn get_record(&self, identity: &Did) -> Result<Option<LinkRecord>>;

    /* Operator APIs */

    /// Connects to peers provided in `add_peers`.
    async fn bootstrap(&self) -> Result<()>;
}

/// Helper macro for running agnostic [NameSystemClient] tests for
/// multiple implementations: [NameSystem] and [HTTPClient].
/// The client is expected to be connected to a bootstrap node
/// in its `before_each()` function.
#[cfg(test)]
#[macro_export]
macro_rules! dht_client_tests {
    ($type:ty, $before_each:ident, $data:ty) => {
        #[tokio::test]
        async fn name_system_client_network_info() -> Result<()> {
            let (_data, client) = $before_each().await?;
            $crate::dht_client::test::test_network_info::<$type>(client).await
        }

        #[tokio::test]
        async fn name_system_client_listeners() -> Result<()> {
            let (_data, client) = $before_each().await?;
            $crate::dht_client::test::test_listeners::<$type>(client).await
        }

        #[tokio::test]
        async fn name_system_client_records() -> Result<()> {
            let (_data, client) = $before_each().await?;
            $crate::dht_client::test::test_records::<$type>(client).await
        }
    };
}

#[cfg(test)]
/// These tests are designed to run on implementations of the
/// NameSystemClient trait, both `NameSystem` and `server::HTTPClient`.
/// Larger intergration tests are found in `tests/ns_test.rs`, and
/// the primary driver of these trait tests are validating
/// the API server functionality in `noosphere_ns::server`.
pub mod test {
    use super::*;
    use crate::{utils::wait_for_peers, NameSystemBuilder};
    use cid::Cid;
    use libp2p::multiaddr::Protocol;
    use noosphere_core::{
        authority::{generate_capability, generate_ed25519_key, SphereAbility},
        data::{Did, LINK_RECORD_FACT_NAME},
        tracing::initialize_tracing,
        view::SPHERE_LIFETIME,
    };
    use noosphere_storage::{MemoryStorage, SphereDb};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial};

    pub async fn test_network_info<C: DhtClient>(client: Arc<Mutex<C>>) -> Result<()> {
        initialize_tracing(None);
        let client = client.lock().await;
        let network_info = client.network_info().await?;
        assert!(network_info.num_connections >= 1);
        Ok(())
    }

    pub async fn test_listeners<C: DhtClient>(client: Arc<Mutex<C>>) -> Result<()> {
        initialize_tracing(None);
        let client = client.lock().await;

        assert!(client.address().await?.is_none());
        let listener_address = client.listen("/ip4/127.0.0.1/tcp/0".parse()?).await?;
        assert_eq!(listener_address, client.address().await?.unwrap());

        match listener_address.iter().collect::<Vec<_>>()[..] {
            [Protocol::Ip4(_ip_addr), Protocol::Tcp(_port), Protocol::P2p(_peer_id)] => {}
            _ => panic!("invalid address {}", listener_address),
        }

        assert_eq!(client.peers().await?.len(), 1);

        // Now test another node connecting.
        let (_other_ns, other_peer_id) = {
            let key_material = generate_ed25519_key();
            let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .ucan_store(store)
                .key_material(&key_material)
                .listening_port(0)
                .bootstrap_peers(&[listener_address.clone()])
                .use_test_config()
                .build()
                .await
                .unwrap();
            ns.bootstrap().await.unwrap();
            let peer_id = ns.peer_id().to_owned();
            (ns, peer_id)
        };

        wait_for_peers::<C>(&client, 2).await?;

        let peers = client.peers().await?;
        assert_eq!(peers.len(), 2);
        assert!(peers.contains(&Peer {
            peer_id: other_peer_id,
        }));

        assert!(client.address().await?.is_some());
        client.stop_listening().await?;
        assert!(client.address().await?.is_none());

        Ok(())
    }

    pub async fn test_records<C: DhtClient>(client: Arc<Mutex<C>>) -> Result<()> {
        initialize_tracing(None);
        let client = client.lock().await;
        client.listen("/ip4/127.0.0.1/tcp/0".parse()?).await?;

        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let link: Cid = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i"
            .parse()
            .unwrap();
        let ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&sphere_identity)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .with_fact(LINK_RECORD_FACT_NAME, link.to_string())
            .with_lifetime(SPHERE_LIFETIME)
            .build()?
            .sign()
            .await?;
        let record = LinkRecord::try_from(ucan)?;

        client.put_record(record, 1).await?;

        let retrieved = client
            .get_record(&sphere_identity)
            .await?
            .expect("should be some");

        assert_eq!(retrieved.to_sphere_identity(), sphere_identity);
        assert_eq!(retrieved.get_link(), Some(link.into()));
        Ok(())
    }
}
