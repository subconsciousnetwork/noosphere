use crate::dht::{
    channel::{Message, MessageClient, MessageProcessor},
    utils,
};
use crate::NameSystemConfig;
use anyhow::{Error, Result};
use libp2p;

pub enum DHTRequest {
    GetRecord { name: Vec<u8> },
    SetRecord { name: Vec<u8>, value: Vec<u8> },
    StartProviding { name: Vec<u8> },
}

pub enum DHTResponse {
    GetRecord { name: Vec<u8>, value: Vec<u8> },
    SetRecord { name: Vec<u8> },
    StartProviding { name: Vec<u8> },
}

pub type DHTMessage = Message<DHTRequest, DHTResponse>;
pub type DHTMessageProcessor = MessageProcessor<DHTRequest, DHTResponse>;
pub type DHTMessageClient = MessageClient<DHTRequest, DHTResponse>;
