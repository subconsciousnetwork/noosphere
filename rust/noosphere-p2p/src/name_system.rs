use crate::address_book::AddressBook;
use crate::dht::{DHTConfig, DHTNode};
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use std::net::SocketAddr;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// [NameSystem] spins up a [DHTNode] in a network for resolving Sphere DIDs into the most
/// recent, verifiable CID for that sphere.
pub struct NameSystem<'a> {
    pub(crate) dht: Option<DHTNode>,
    pub(crate) dht_config: DHTConfig,
    pub(crate) address_book: Option<AddressBook>,
    pub(crate) key_material: &'a Ed25519KeyMaterial,
    pub(crate) listening_address: SocketAddr,
    pub(crate) bootstrap_peers: Option<&'a Vec<String>>,
}

impl<'a> NameSystem<'a> {
    /// Initializes and attempts to connect to the network.
    pub async fn connect(&mut self) -> Result<()> {
        self.dht = Some(DHTNode::new(
            self.key_material,
            &self.listening_address,
            self.bootstrap_peers,
            &self.dht_config,
        )?);
        self.provide_address_book().await?;
        Ok(())
    }

    /// Disconnect and deallocate connections to the network.
    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(mut dht) = self.dht.take() {
            dht.terminate()?;
        }
        Ok(())
    }

    /// Get record associated with `key`.
    pub async fn get_record(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>> {
        let dht = self.dht.as_ref().ok_or_else(|| anyhow!("not connected"))?;

        dht.get_record(key)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    /// Stores record `value` associated with `key`.
    pub async fn set_record(&self, key: Vec<u8>, value: Vec<u8>) -> Result<Vec<u8>> {
        let dht = self.dht.as_ref().ok_or_else(|| anyhow!("not connected"))?;

        dht.set_record(key, value)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub fn p2p_address(&self) -> Option<&libp2p::Multiaddr> {
        if let Some(dht) = &self.dht {
            Some(dht.p2p_address())
        } else {
            None
        }
    }

    async fn provide_address_book(&self) -> Result<()> {
        if self.address_book.is_none() {
            return Ok(());
        }
        let dht = self.dht.as_ref().ok_or_else(|| anyhow!("not connected"))?;
        info!("BOOTSTRAPPING");
        dht.bootstrap().await?;
        info!("WAITING FOR PEERS!");
        dht.wait_for_peers(1).await?;
        info!("PEERS! {:#?}", dht.network_info().await?);
        let address_book = self.address_book.as_ref().unwrap();
        let pending_tasks: Vec<_> = address_book
            .iter()
            .map(|record| dht.set_record(record.key.clone(), record.value.clone()))
            .collect();
        let results = try_join_all(pending_tasks).await?;
        for result in results {
            info!(
                "Set record for {}",
                String::from_utf8(result).map_err(|e| anyhow!(e.to_string()))?
            );
        }
        Ok(())
    }
}
/*
impl Drop for NameSystem {
    fn drop(&mut self) {
        if let Err(e) = self.disconnect() {
            error!("{}", e.to_string());
        }
    }
}
*/
