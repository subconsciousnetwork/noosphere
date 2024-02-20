use super::{Ability, CapabilitySemantics, Scope};
use anyhow::{anyhow, Result};
use std::fmt;
use url::Url;

#[derive(Ord, Eq, PartialEq, PartialOrd, Clone)]
pub enum ProofAction {
    Delegate,
}

impl Ability for ProofAction {}

impl TryFrom<String> for ProofAction {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        match value.as_str() {
            "ucan/DELEGATE" => Ok(ProofAction::Delegate),
            unsupported => Err(anyhow!(
                "Unsupported action for proof resource ({})",
                unsupported
            )),
        }
    }
}

impl fmt::Display for ProofAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ProofAction::Delegate => "ucan/DELEGATE",
            }
        )
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum ProofSelection {
    Index(usize),
    All,
}

impl Scope for ProofSelection {
    fn contains(&self, other: &Self) -> bool {
        self == other || *self == ProofSelection::All
    }
}

impl TryFrom<Url> for ProofSelection {
    type Error = anyhow::Error;

    fn try_from(value: Url) -> Result<Self, Self::Error> {
        match value.scheme() {
            "prf" => String::from(value.path()).try_into(),
            _ => Err(anyhow!("Unrecognized URI scheme")),
        }
    }
}

impl TryFrom<String> for ProofSelection {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        match value.as_str() {
            "*" => Ok(ProofSelection::All),
            selection => Ok(ProofSelection::Index(selection.parse::<usize>()?)),
        }
    }
}

impl fmt::Display for ProofSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofSelection::Index(usize) => write!(f, "prf:{}", usize),
            ProofSelection::All => write!(f, "prf:*"),
        }
    }
}

pub struct ProofDelegationSemantics {}

impl CapabilitySemantics<ProofSelection, ProofAction> for ProofDelegationSemantics {}
