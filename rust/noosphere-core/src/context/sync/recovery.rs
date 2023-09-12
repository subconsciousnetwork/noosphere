/// Recovery strategies for cases when gateway synchronization fails but may be
/// able to recover gracefully (e.g., when the gateway reports a conflict).
#[derive(Debug)]
pub enum SyncRecovery {
    /// Do not attempt to recover
    None,
    /// Automatically retry the synchronization up to a certain number of times
    Retry(u32),
}
