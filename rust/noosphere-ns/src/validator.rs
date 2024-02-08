use crate::dht::Validator;
use async_trait::async_trait;
use noosphere_core::data::LinkRecord;
use noosphere_ucan::store::UcanStore;

/// Implements [Validator] for the DHT.
pub(crate) struct RecordValidator<S: UcanStore> {
    store: S,
}

impl<S> RecordValidator<S>
where
    S: UcanStore,
{
    pub fn new(store: S) -> Self {
        RecordValidator { store }
    }
}

#[async_trait]
impl<S> Validator for RecordValidator<S>
where
    S: UcanStore,
{
    async fn validate(&mut self, record_value: &[u8]) -> bool {
        match LinkRecord::try_from(record_value) {
            Ok(record) => record.validate(&self.store).await.is_ok(),
            _ => false,
        }
    }
}
