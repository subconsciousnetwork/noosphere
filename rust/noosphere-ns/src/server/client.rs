use crate::server::routes::Route;
use crate::{dht_client::DhtClient, Multiaddr, NetworkInfo, Peer, PeerId};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::data::{Did, LinkRecord};
use reqwest::Body;
use url::Url;

pub struct HttpClient {
    api_base: Url,
    client: reqwest::Client,
    peer_id: PeerId,
}

impl HttpClient {
    pub async fn new(api_base: Url) -> Result<Self> {
        let client = reqwest::Client::new();
        let mut url = api_base.clone();
        url.set_path(&Route::GetPeerId.to_string());
        let peer_id = client.get(url).send().await?.json().await?;

        Ok(HttpClient {
            api_base,
            client,
            peer_id,
        })
    }
}

#[async_trait]
impl DhtClient for HttpClient {
    /// Returns current network information for this node.
    async fn network_info(&self) -> Result<NetworkInfo> {
        let mut url = self.api_base.clone();
        url.set_path(&Route::NetworkInfo.to_string());
        Ok(self.client.get(url).send().await?.json().await?)
    }

    /// Returns the current peers of this node.
    fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Returns the current peers of this node.
    async fn peers(&self) -> Result<Vec<Peer>> {
        let mut url = self.api_base.clone();
        url.set_path(&Route::GetPeers.to_string());
        Ok(self.client.get(url).send().await?.json().await?)
    }

    /// Adds peers to connect to. Unless bootstrapping a network, at least one
    /// peer is needed.
    async fn add_peers(&self, peers: Vec<Multiaddr>) -> Result<()> {
        if peers.len() > 1 {
            return Err(anyhow!("Only one peer may be added at a time over HTTP."));
        }
        let address = peers.first().unwrap();
        let mut url = self.api_base.clone();
        let path = Route::AddPeers
            .to_string()
            .replace("*addr", &address.to_string());
        url.set_path(&path);
        Ok(self.client.post(url).send().await?.json().await?)
    }

    /// Starts listening for connections on provided address.
    async fn listen(&self, listening_address: Multiaddr) -> Result<Multiaddr> {
        let mut url = self.api_base.clone();
        let path = Route::Listen
            .to_string()
            .replace("*addr", &listening_address.to_string());
        url.set_path(&path);
        Ok(self.client.post(url).send().await?.json().await?)
    }

    /// Stops listening for connections.
    async fn stop_listening(&self) -> Result<()> {
        let mut url = self.api_base.clone();
        let path = Route::StopListening.to_string();
        url.set_path(&path);
        Ok(self.client.delete(url).send().await?.json().await?)
    }

    /// Begins the bootstrap process on this node.
    async fn bootstrap(&self) -> Result<()> {
        let mut url = self.api_base.clone();
        url.set_path(&Route::Bootstrap.to_string());
        Ok(self.client.post(url).send().await?.json().await?)
    }

    /// Returns the listening addresses of this node.
    async fn address(&self) -> Result<Option<Multiaddr>> {
        let mut url = self.api_base.clone();
        url.set_path(&Route::Address.to_string());
        Ok(self.client.get(url).send().await?.json().await?)
    }

    async fn get_record(&self, identity: &Did) -> Result<Option<LinkRecord>> {
        let mut url = self.api_base.clone();
        let path = Route::GetRecord
            .to_string()
            .replace(":identity", identity.into());
        url.set_path(&path);
        Ok(self.client.get(url).send().await?.json().await?)
    }

    async fn put_record(&self, record: LinkRecord, quorum: usize) -> Result<()> {
        let mut url = self.api_base.clone();
        url.set_path(&Route::PostRecord.to_string());
        url.set_query(Some(&format!("quorum={quorum}")));
        let json_data = serde_json::to_string(&record)?;

        let res = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(Body::from(json_data))
            .send()
            .await?
            .json()
            .await?;
        Ok(res)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::dht_client_tests;
    use crate::{server::ApiServer, utils::wait_for_peers};
    use crate::{NameSystem, NameSystemBuilder};
    use noosphere_core::authority::generate_ed25519_key;
    use noosphere_storage::{MemoryStorage, SphereDb};
    use std::net::TcpListener;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// This struct is used to persist non-Client objects, like
    /// the name system and/or server, through the duration
    /// of each test.
    struct DataPlaceholder {
        _server: ApiServer,
        _bootstrap: NameSystem,
    }

    async fn before_each() -> Result<(DataPlaceholder, Arc<Mutex<HttpClient>>)> {
        let (bootstrap, bootstrap_address) = {
            let key_material = generate_ed25519_key();
            let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .ucan_store(store)
                .key_material(&key_material)
                .listening_port(0)
                .use_test_config()
                .build()
                .await
                .unwrap();
            ns.bootstrap().await.unwrap();
            let address = ns.address().await?.unwrap();
            (ns, vec![address])
        };

        let api_listener = TcpListener::bind("127.0.0.1:0")?;
        let api_port = api_listener.local_addr().unwrap().port();
        let api_url = Url::parse(&format!("http://127.0.0.1:{}", api_port))?;
        let key_material = generate_ed25519_key();
        let store = SphereDb::new(MemoryStorage::default()).await.unwrap();

        let ns = NameSystemBuilder::default()
            .ucan_store(store)
            .key_material(&key_material)
            .bootstrap_peers(&bootstrap_address)
            .use_test_config()
            .build()
            .await
            .unwrap();

        let ns = Arc::new(ns);
        let server = ApiServer::serve(ns, api_listener);
        let data = DataPlaceholder {
            _server: server,
            _bootstrap: bootstrap,
        };

        let client = {
            let client = HttpClient::new(api_url).await?;
            // Bootstrap via the HTTP client to test the route
            client.bootstrap().await?;
            wait_for_peers::<HttpClient>(&client, 1).await?;
            Arc::new(Mutex::new(client))
        };

        Ok((data, client))
    }

    dht_client_tests!(HttpClient, before_each, DataPlaceholder);
}
