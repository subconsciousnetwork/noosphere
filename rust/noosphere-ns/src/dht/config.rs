use serde::Deserialize;

#[cfg(doc)]
use libp2p::kad::KademliaConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct DhtConfig {
    /// If bootstrap peers are provided, how often,
    /// in seconds, should the bootstrap process execute
    /// to keep routing tables fresh.
    pub bootstrap_interval: u64,
    /// How frequently, in seconds, the DHT attempts to
    /// dial peers found in its kbucket. Outside of tests,
    /// should not be lower than 5 seconds.
    pub peer_dialing_interval: u64,
    /// How long, in seconds, published records are replicated to
    /// peers. Should be significantly shorter than `record_ttl`.
    /// See [KademliaConfig::set_publication_interval] and [KademliaConfig::set_provider_publication_interval].
    pub publication_interval: u32,
    /// How long, in seconds, until an unsuccessful
    /// DHT query times out.
    pub query_timeout: u32,
    /// How long, in seconds, stored records are replicated to
    /// peers. Should be significantly shorter than `publication_interval`.
    /// See [KademliaConfig::set_replication_interval].
    /// Only applies to value records.
    pub replication_interval: u32,
    /// How long, in seconds, records remain valid for. Should be significantly
    /// longer than `publication_interval`.
    /// See [KademliaConfig::set_record_ttl] and [KademliaConfig::set_provider_record_ttl].
    pub record_ttl: u32,
}

impl Default for DhtConfig {
    /// Creates a new [DHTConfig] with defaults applied.
    fn default() -> Self {
        let peer_dialing_interval = if cfg!(test) { 1 } else { 5 };
        Self {
            bootstrap_interval: 5 * 60, // 5 mins
            peer_dialing_interval,
            publication_interval: 60 * 60 * 24, // 1 day
            query_timeout: 5 * 60,              // 5 mins
            replication_interval: 60 * 60,      // 1 hour
            record_ttl: 60 * 60 * 24 * 3,       // 3 days
        }
    }
}
