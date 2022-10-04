use crate::dht::channel::{Message, MessageClient, MessageProcessor};
use libp2p;

pub enum DHTRequest {
    GetRecord(Vec<u8>),
    SetRecord { name: Vec<u8>, value: Vec<u8> },
}

pub enum DHTResponse {
    GetRecord { name: Vec<u8>, value: Vec<u8> },
    SetRecord { name: Vec<u8> },
}

pub type DHTMessage = Message<DHTRequest, DHTResponse>;
pub type DHTMessageProcessor = MessageProcessor<DHTRequest, DHTResponse>;
pub type DHTMessageClient = MessageClient<DHTRequest, DHTResponse>;

#[derive(Clone)]
pub struct DHTConfig {
    pub keypair: libp2p::identity::Keypair,
    pub query_timeout: u32,
}
