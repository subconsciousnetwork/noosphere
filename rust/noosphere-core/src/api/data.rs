use std::str::FromStr;

use anyhow::Result;

use serde::{Deserialize, Deserializer};

/// A helper to express the deserialization of a query string to some
/// consistent result type
pub trait AsQuery {
    /// Get the value of this trait implementor as a [Result<Option<String>>]
    fn as_query(&self) -> Result<Option<String>>;
}

impl AsQuery for () {
    fn as_query(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

// NOTE: Adapted from https://github.com/tokio-rs/axum/blob/7caa4a3a47a31c211d301f3afbc518ea2c07b4de/examples/query-params-with-empty-strings/src/main.rs#L42-L54
/// Serde deserialization decorator to map empty Strings to None,
pub(crate) fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s)
            .map_err(serde::de::Error::custom)
            .map(Some),
    }
}
