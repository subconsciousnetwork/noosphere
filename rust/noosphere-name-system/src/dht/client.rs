//use async_std::task;
use crate::dht::{
    channel::message_channel,
    errors::DHTError,
    node::DHTNode,
    types::{DHTMessageClient, DHTNetworkInfo, DHTRequest, DHTResponse, DHTStatus},
    DHTConfig,
};
use tokio;

macro_rules! ensure_response {
    ($response:expr, $matcher:pat => $statement:expr) => {
        match $response {
            $matcher => $statement,
            _ => Err(DHTError::Error("Unexpected".into())),
        }
    };
}

/// Primary interface for the DHT swarm.
pub struct DHTClient {
    config: DHTConfig,
    state: DHTStatus,
    client: Option<DHTMessageClient>,
    handle: Option<tokio::task::JoinHandle<Result<(), DHTError>>>,
}

impl DHTClient {
    pub fn new(config: DHTConfig) -> Self {
        DHTClient {
            config,
            state: DHTStatus::Inactive,
            client: None,
            handle: None,
        }
    }

    /// Begin the network processing thread and start handling
    /// network requests.
    pub async fn start(&mut self) -> Result<(), DHTError> {
        let (client, processor) = message_channel::<DHTRequest, DHTResponse, DHTError>();
        self.client = Some(client);
        self.handle = Some(DHTNode::spawn(self.config.clone(), processor)?);
        self.state = DHTStatus::Active;
        Ok(())
    }

    /// Teardown the network processing thread.
    /// @TODO Anything to wait on for thread cleanup?
    pub async fn stop(&mut self) -> Result<(), DHTError> {
        if self.handle.is_some() {
            self.handle.as_ref().expect("JoinHandle exists").abort();
            self.handle = None;
        }
        if self.client.is_some() {
            self.client = None;
        }
        self.state = DHTStatus::Inactive;
        Ok(())
    }

    pub fn status(&self) -> DHTStatus {
        self.state.clone()
    }

    pub async fn bootstrap(&self) -> Result<DHTNetworkInfo, DHTError> {
        let request = DHTRequest::Bootstrap;
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::Bootstrap(info) => Ok(info))
    }

    pub async fn network_info(&self) -> Result<DHTNetworkInfo, DHTError> {
        let request = DHTRequest::GetNetworkInfo;
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetNetworkInfo(info) => Ok(info))
    }

    pub async fn start_providing(&self, name: Vec<u8>) -> Result<(), DHTError> {
        let request = DHTRequest::StartProviding { name };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::StartProviding { name } => Ok(()))
    }

    pub async fn set_record(&self, name: Vec<u8>, value: Vec<u8>) -> Result<Vec<u8>, DHTError> {
        let request = DHTRequest::SetRecord { name, value };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::SetRecord { name } => Ok(name))
    }

    pub async fn get_record(&self, name: Vec<u8>) -> Result<Option<Vec<u8>>, DHTError> {
        let request = DHTRequest::GetRecord { name };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetRecord { value, .. } => Ok(Some(value)))
    }

    async fn send_request(&self, request: DHTRequest) -> Result<DHTResponse, DHTError> {
        self.ensure_active()?;
        let client = self.client.as_ref().unwrap();
        client
            .send_request_async(request)
            .await
            .map_err(|e| DHTError::from(e))
            .and_then(|res| res)
    }

    fn ensure_active(&self) -> Result<(), DHTError> {
        match self.state {
            DHTStatus::Active => {
                assert!(self.client.is_some(), "Client must exist in active DHT.");
                assert!(self.handle.is_some(), "Handle must exist in active DHT.");
                Ok(())
            }
            _ => Err(DHTError::NotConnected),
        }
    }
}

impl Drop for DHTClient {
    fn drop(&mut self) {
        debug!("Dropping DHTClient");
    }
}
