use std::{convert::TryFrom, fmt::Display, str::FromStr};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_core::{ipld::Ipld, raw::RawCodec};
use noosphere_storage::block_encode;
use ucan::{store::UcanJwtStore, Ucan};

#[cfg(doc)]
use ucan::ipld::UcanIpld;

#[cfg(doc)]
use crate::data::Jwt;

/// An [Authorization] is a wrapper around something that can be resolved into
/// a [Ucan]. Typically this is a [Cid], but it may also be something like a
/// [Ucan] itself, or a [UcanIpld], or a [Jwt]. We don't want to deal with each
/// of these heterogenous types separately, so we move them around as an
/// [Authorization] instead.
/// TODO(ucan-wg/rs-ucan#32): Maybe swap this out is we get a substantially
/// similar construct to land in rs-ucan
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
            Authorization::Cid(cid) => Ucan::from_str(&store.require_token(cid).await?),
        }
    }
}

impl FromStr for Authorization {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.trim().split(':');
        Ok(match parts.next() {
            Some("jwt") => Authorization::Ucan(Ucan::from_str(
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

impl From<Cid> for Authorization {
    fn from(cid: Cid) -> Self {
        Authorization::Cid(cid)
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

impl Display for Authorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cid = Cid::try_from(self).map_err(|_| std::fmt::Error)?;
        cid.fmt(f)
    }
}
