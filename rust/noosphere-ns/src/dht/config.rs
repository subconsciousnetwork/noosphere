#[derive(Clone, Debug)]
pub struct DHTConfig {
    /// If bootstrap peers are provided, how often,
    /// in seconds, should the bootstrap process execute
    /// to keep routing tables fresh.
    pub bootstrap_interval: u64,
    /// How frequently, in seconds, the DHT attempts to
    /// dial peers found in its kbucket. Outside of tests,
    /// should not be lower than 5 seconds.
    pub peer_dialing_interval: u64,
    /// How long, in seconds, until an unsuccessful
    /// DHT query times out.
    pub query_timeout: u32,
}

impl Default for DHTConfig {
    /// Creates a new [DHTConfig] with defaults applied.
    fn default() -> Self {
        Self {
            bootstrap_interval: 5 * 60,
            peer_dialing_interval: 5,
            query_timeout: 5 * 60,
        }
    }
}
