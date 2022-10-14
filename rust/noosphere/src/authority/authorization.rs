use std::{convert::TryFrom, str::FromStr};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_core::{ipld::Ipld, raw::RawCodec};
use noosphere_storage::encoding::block_encode;
use ucan::{store::UcanJwtStore, Ucan};

// TODO(ucan-wg/rs-ucan#32): Maybe swap this out is we get a substantially
// similar construct to land in rs-ucan
#[derive(Clone)]
pub enum Authorization {
    /// A fully instantiated UCAN
    Ucan(Ucan),
    /// A CID that refers to a UCAN that may be looked up in storage at the
    /// point of invocation
    Cid(Cid),
}

impl Authorization {
    pub async fn resolve_ucan<S: UcanJwtStore>(&self, store: &S) -> Result<Ucan> {
        match self {
            Authorization::Ucan(ucan) => Ok(ucan.clone()),
            Authorization::Cid(cid) => {
                Ucan::try_from_token_string(&store.require_token(cid).await?)
            }
        }
    }
}

impl FromStr for Authorization {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.trim().split(':');
        Ok(match parts.next() {
            Some("jwt") => Authorization::Ucan(Ucan::try_from_token_string(
                parts.next().ok_or_else(|| anyhow!("Missing token"))?,
            )?),
            Some("cid") => Authorization::Cid(Cid::try_from(
                parts.next().ok_or_else(|| anyhow!("Missing CID string"))?,
            )?),
            Some(any_other) => Authorization::Cid(Cid::from_str(any_other)?),
            None => return Err(anyhow!("Authorization had empty value")),
        })
    }
}

impl TryFrom<Authorization> for Cid {
    type Error = anyhow::Error;

    fn try_from(value: Authorization) -> Result<Self, Self::Error> {
        Cid::try_from(&value)
    }
}

impl TryFrom<&Authorization> for Cid {
    type Error = anyhow::Error;

    fn try_from(value: &Authorization) -> Result<Self, Self::Error> {
        Ok(match value {
            Authorization::Ucan(ucan) => {
                let jwt = ucan.encode()?;
                let (cid, _) = block_encode::<RawCodec, _>(&Ipld::Bytes(jwt.as_bytes().to_vec()))?;
                cid
            }
            Authorization::Cid(cid) => *cid,
        })
    }
}
