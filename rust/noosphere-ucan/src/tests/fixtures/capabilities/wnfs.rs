use crate::capability::{Ability, CapabilitySemantics, Scope};
use anyhow::{anyhow, Result};
use url::Url;

#[derive(Ord, Eq, PartialOrd, PartialEq, Clone)]
pub enum WNFSCapLevel {
    Create,
    Revise,
    SoftDelete,
    Overwrite,
    SuperUser,
}

impl Ability for WNFSCapLevel {}

impl TryFrom<String> for WNFSCapLevel {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(match value.as_str() {
            "wnfs/create" => WNFSCapLevel::Create,
            "wnfs/revise" => WNFSCapLevel::Revise,
            "wnfs/soft_delete" => WNFSCapLevel::SoftDelete,
            "wnfs/overwrite" => WNFSCapLevel::Overwrite,
            "wnfs/super_user" => WNFSCapLevel::SuperUser,
            _ => return Err(anyhow!("No such WNFS capability level: {}", value)),
        })
    }
}

impl ToString for WNFSCapLevel {
    fn to_string(&self) -> String {
        match self {
            WNFSCapLevel::Create => "wnfs/create",
            WNFSCapLevel::Revise => "wnfs/revise",
            WNFSCapLevel::SoftDelete => "wnfs/soft_delete",
            WNFSCapLevel::Overwrite => "wnfs/overwrite",
            WNFSCapLevel::SuperUser => "wnfs/super_user",
        }
        .into()
    }
}

#[derive(Clone, PartialEq)]
pub struct WNFSScope {
    origin: String,
    path: String,
}

impl Scope for WNFSScope {
    fn contains(&self, other: &Self) -> bool {
        if self.origin != other.origin {
            return false;
        }

        let self_path_parts = self.path.split('/');
        let mut other_path_parts = other.path.split('/');

        for part in self_path_parts {
            match other_path_parts.nth(0) {
                Some(other_part) => {
                    if part != other_part {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }
}

impl TryFrom<Url> for WNFSScope {
    type Error = anyhow::Error;

    fn try_from(value: Url) -> Result<Self, Self::Error> {
        match (value.scheme(), value.host_str(), value.path()) {
            ("wnfs", Some(host), path) => Ok(WNFSScope {
                origin: String::from(host),
                path: String::from(path),
            }),
            _ => Err(anyhow!("Cannot interpret URI as WNFS scope: {}", value)),
        }
    }
}

impl ToString for WNFSScope {
    fn to_string(&self) -> String {
        format!("wnfs://{}{}", self.origin, self.path)
    }
}

pub struct WNFSSemantics {}

impl CapabilitySemantics<WNFSScope, WNFSCapLevel> for WNFSSemantics {}
