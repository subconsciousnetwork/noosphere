//use async_std::task;
use crate::dht::{
    channel::message_channel,
    node::DHTNode,
    types::{DHTMessageClient, DHTRequest, DHTResponse},
    DHTConfig,
};
use anyhow::{anyhow, Result};
use tokio;

#[derive(PartialEq, Eq)]
pub enum DHTState {
    Initialized,
    Connected,
    Disconnected,
}

/// Primary interface for the DHT swarm.
pub struct DHTClient {
    config: DHTConfig,
    state: DHTState,
    client: Option<DHTMessageClient>,
    handle: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl DHTClient {
    pub fn new(config: DHTConfig) -> Self {
        DHTClient {
            config,
            state: DHTState::Initialized,
            client: None,
            handle: None,
        }
    }

    /// Connect to DHT network.
    pub async fn connect(&mut self) -> Result<()> {
        let (client, processor) = message_channel::<DHTRequest, DHTResponse>();
        self.client = Some(client);
        self.handle = Some(DHTNode::spawn(self.config.clone(), processor)?);
        self.state = DHTState::Connected;
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.ensure_connected()?;
        if self.handle.is_some() {
            self.handle.as_ref().expect("JoinHandle exists").abort();
            self.handle = None;
        }
        self.state = DHTState::Disconnected;
        Ok(())
    }

    pub async fn set_record(&self, name: Vec<u8>, value: Vec<u8>) -> Result<Vec<u8>> {
        let request = DHTRequest::SetRecord { name, value };
        let response = self.send_request(request).await?;
        match response {
            DHTResponse::SetRecord { name } => Ok(name),
            _ => Err(anyhow!("Unexpected response.")),
        }
    }

    pub async fn get_record(&self, name: Vec<u8>) -> Result<Option<Vec<u8>>> {
        let request = DHTRequest::GetRecord { name };
        let response = self.send_request(request).await?;
        match response {
            DHTResponse::GetRecord { value, .. } => Ok(Some(value)),
            _ => Err(anyhow!("unexpected response.")),
        }
    }

    async fn send_request(&self, request: DHTRequest) -> Result<DHTResponse> {
        self.ensure_connected()?;
        let client = self.client.as_ref().unwrap();
        client.send_request_async(request).await
    }

    fn ensure_connected(&self) -> Result<()> {
        if self.state != DHTState::Connected {
            Err(anyhow!("DHT not connected."))
        } else if self.client.is_none() {
            Err(anyhow!("DHT not connected"))
        } else {
            Ok(())
        }
    }
}
