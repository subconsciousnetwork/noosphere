use crate::dht::RecordValidator;
use crate::records::NSRecord;
use async_trait::async_trait;
use noosphere_core::authority::SUPPORTED_KEYS;
use noosphere_storage::{SphereDb, Storage};
use ucan::crypto::did::DidParser;

pub struct Validator<S: Storage> {
    store: SphereDb<S>,
    did_parser: DidParser,
}

impl<S> Validator<S>
where
    S: Storage,
{
    pub fn new(store: &SphereDb<S>) -> Self {
        Validator {
            store: store.to_owned(),
            did_parser: DidParser::new(SUPPORTED_KEYS),
        }
    }
}

#[async_trait]
impl<S> RecordValidator for Validator<S>
where
    S: Storage,
{
    async fn validate(&mut self, record_value: &[u8]) -> bool {
        if let Ok(record) = NSRecord::try_from(record_value) {
            return record
                .validate(&self.store, &mut self.did_parser)
                .await
                .is_ok();
        }
        return false;
    }
}
