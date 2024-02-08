use serde::Deserialize;

#[cfg(doc)]
use libp2p::kad::Config as KademliaConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct DhtConfig {
    /// If bootstrap peers are provided, how often,
    /// in seconds, should the bootstrap process execute
    /// to keep routing tables fresh.
    #[serde(default = "default_bootstrap_interval")]
    pub bootstrap_interval: u64,
    /// How frequently, in seconds, the DHT attempts to
    /// dial peers found in its kbucket. Outside of tests,
    /// should not be lower than 5 seconds.
    #[serde(default = "default_peer_dialing_interval")]
    pub peer_dialing_interval: u64,
    /// How long, in seconds, published records are replicated to
    /// peers. Should be significantly shorter than `record_ttl`.
    /// See [KademliaConfig::set_publication_interval] and [KademliaConfig::set_provider_publication_interval].
    #[serde(default = "default_publication_interval")]
    pub publication_interval: u32,
    /// How long, in seconds, until an unsuccessful
    /// DHT query times out.
    #[serde(default = "default_query_timeout")]
    pub query_timeout: u32,
    /// How long, in seconds, stored records are replicated to
    /// peers. Should be significantly shorter than `publication_interval`.
    /// See [KademliaConfig::set_replication_interval].
    /// Only applies to value records.
    #[serde(default = "default_replication_interval")]
    pub replication_interval: u32,
    /// How long, in seconds, records remain valid for. Should be significantly
    /// longer than `publication_interval`.
    /// See [KademliaConfig::set_record_ttl] and [KademliaConfig::set_provider_record_ttl].
    #[serde(default = "default_record_ttl")]
    pub record_ttl: u32,
}

// We break up defaults into individual functions to support deserializing
// via serde when `DhtConfig` is used as a nested value. Otherwise,
// `[dht_config] query_timeout = 60` would require defining all other fields.

fn default_bootstrap_interval() -> u64 {
    5 * 60 // 5 mins
}

fn default_peer_dialing_interval() -> u64 {
    if cfg!(test) {
        1
    } else {
        5
    }
}

fn default_publication_interval() -> u32 {
    60 * 60 * 24 // 1 day
}

fn default_query_timeout() -> u32 {
    5 * 60 // 5 mins
}

fn default_replication_interval() -> u32 {
    60 * 60 // 1 hour
}

fn default_record_ttl() -> u32 {
    60 * 60 * 24 * 3 // 3 days
}

impl Default for DhtConfig {
    /// Creates a new [DhtConfig] with defaults applied.
    fn default() -> Self {
        Self {
            bootstrap_interval: default_bootstrap_interval(),
            peer_dialing_interval: default_peer_dialing_interval(),
            publication_interval: default_publication_interval(),
            query_timeout: default_query_timeout(),
            replication_interval: default_replication_interval(),
            record_ttl: default_record_ttl(),
        }
    }
}
