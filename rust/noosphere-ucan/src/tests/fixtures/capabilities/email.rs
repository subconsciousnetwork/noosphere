use crate::capability::{Ability, CapabilitySemantics, Scope};
use anyhow::{anyhow, Result};
use url::Url;

#[derive(Clone, PartialEq)]
pub struct EmailAddress(String);

impl Scope for EmailAddress {
    fn contains(&self, other: &Self) -> bool {
        return self.0 == other.0;
    }
}

impl ToString for EmailAddress {
    fn to_string(&self) -> String {
        format!("mailto:{}", self.0.clone())
    }
}

impl TryFrom<Url> for EmailAddress {
    type Error = anyhow::Error;

    fn try_from(value: Url) -> Result<Self> {
        match value.scheme() {
            "mailto" => Ok(EmailAddress(String::from(value.path()))),
            _ => Err(anyhow!(
                "Could not interpret URI as an email address: {}",
                value
            )),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum EmailAction {
    Send,
}

impl Ability for EmailAction {}

impl ToString for EmailAction {
    fn to_string(&self) -> String {
        match self {
            EmailAction::Send => "email/send",
        }
        .into()
    }
}

impl TryFrom<String> for EmailAction {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        match value.as_str() {
            "email/send" => Ok(EmailAction::Send),
            _ => Err(anyhow!("Unrecognized action: {}", value)),
        }
    }
}

pub struct EmailSemantics {}

impl CapabilitySemantics<EmailAddress, EmailAction> for EmailSemantics {}
