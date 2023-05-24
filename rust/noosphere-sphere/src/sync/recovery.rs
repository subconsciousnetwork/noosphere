#[derive(Debug)]
pub enum SyncRecovery {
    None,
    Retry(u32),
}
