use std::{fmt::Display, ops::Deref};

use serde::{Deserialize, Serialize};

/// A DID, aka a Decentralized Identifier, is a string that can be parsed and
/// resolved into a so-called DID Document, usually in order to obtain PKI
/// details related to a particular user or process.
///
/// See: https://en.wikipedia.org/wiki/Decentralized_identifier
/// See: https://www.w3.org/TR/did-core/
#[repr(transparent)]
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Did(pub String);

impl Deref for Did {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for Did {
    fn from(value: &str) -> Self {
        Did(value.to_owned())
    }
}

impl From<String> for Did {
    fn from(value: String) -> Self {
        Did(value)
    }
}

impl From<Did> for String {
    fn from(value: Did) -> Self {
        value.0
    }
}

impl PartialEq<String> for Did {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<Did> for String {
    fn eq(&self, other: &Did) -> bool {
        self == &other.0
    }
}

impl Display for Did {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
