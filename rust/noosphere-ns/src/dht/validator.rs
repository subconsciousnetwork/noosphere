use std::fmt::Display;

use async_trait::async_trait;
/// Trait that implements a `validate` function that determines
/// what records can be set and stored on the [crate::dht::DHTNode].
/// Currently only validates "Value" records.
///
/// # Example
///
/// ```
/// use noosphere_ns::dht::RecordValidator;
/// use async_trait::async_trait;
/// use tokio;
///
/// #[derive(Clone)]
/// struct MyValidator;
///
/// #[async_trait]
/// impl RecordValidator for MyValidator {
///     // Ensures value is "hello" in bytes.
///     async fn validate(&mut self, data: &[u8]) -> bool {
///         data[..] == [104, 101, 108, 108, 111][..]
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut validator = MyValidator {};
///     let data = String::from("hello").into_bytes();
///     let is_valid = validator.validate(&data).await;
///     assert!(is_valid);
/// }
#[async_trait]
pub trait RecordValidator: Send + Sync + Clone {
    async fn validate(&mut self, record_value: &[u8]) -> bool;
}

/// An implementation of [RecordValidator] that allows all records.
/// Used for tests.
#[derive(Clone)]
pub struct AllowAllValidator {}

#[async_trait]
impl RecordValidator for AllowAllValidator {
    async fn validate(&mut self, _data: &[u8]) -> bool {
        true
    }
}

impl Display for AllowAllValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AllowAllValidator")
    }
}
