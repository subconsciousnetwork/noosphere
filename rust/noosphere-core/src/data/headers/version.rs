use anyhow::anyhow;
use std::{convert::Infallible, fmt::Display, str::FromStr};

pub enum Version {
    V0,
    Unknown(String),
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Version::V0 => "0",
            Version::Unknown(header) => header.as_str(),
        };

        write!(f, "{}", value)
    }
}

impl FromStr for Version {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "0" => Version::V0,
            _ => Version::Unknown(String::from(s)),
        })
    }
}

impl TryFrom<Version> for u32 {
    type Error = anyhow::Error;

    fn try_from(value: Version) -> Result<Self, Self::Error> {
        match value {
            Version::V0 => Ok(0),
            Version::Unknown(version) => Err(anyhow!("Unrecognized version: {}", version)),
        }
    }
}
