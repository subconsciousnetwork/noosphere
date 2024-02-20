use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_core::{ipld::Ipld, raw::RawCodec};
use noosphere_storage::block_encode;
use noosphere_ucan::{chain::ProofChain, crypto::did::DidParser, store::UcanJwtStore, Ucan};
use std::{fmt::Display, str::FromStr};

#[cfg(doc)]
use noosphere_ucan::ipld::UcanIpld;

#[cfg(doc)]
use crate::data::Jwt;

use super::SUPPORTED_KEYS;

/// An [Authorization] is a wrapper around something that can be resolved into
/// a [Ucan]. Typically this is a [Cid], but it may also be something like a
/// [Ucan] itself, or a [UcanIpld], or a [Jwt]. We don't want to deal with each
/// of these heterogenous types separately, so we move them around as an
/// [Authorization] instead.
/// TODO(ucan-wg/rs-ucan#32): Maybe swap this out is we get a substantially
/// similar construct to land in rs-ucan
#[derive(Clone, Debug)]
pub enum Authorization {
    /// A fully instantiated [Ucan]
    Ucan(Ucan),
    /// A [Cid] that refers to a [Ucan] that may be looked up in storage at the
    /// point of invocation
    Cid(Cid),
}

impl Authorization {
    /// Attempt to resolve the [Authorization] as a fully deserialized [Ucan]
    /// (if it is not one already).
    pub async fn as_ucan<S: UcanJwtStore>(&self, store: &S) -> Result<Ucan> {
        match self {
            Authorization::Ucan(ucan) => Ok(ucan.clone()),
            Authorization::Cid(cid) => Ucan::from_str(&store.require_token(cid).await?),
        }
    }

    /// Attempt to resolve the [Authorization] as a [ProofChain] (via its associated [Ucan])
    pub async fn as_proof_chain<S: UcanJwtStore>(&self, store: &S) -> Result<ProofChain> {
        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        Ok(match self {
            Authorization::Ucan(ucan) => {
                ProofChain::from_ucan(ucan.clone(), None, &mut did_parser, store).await?
            }
            Authorization::Cid(cid) => {
                ProofChain::from_cid(cid, None, &mut did_parser, store).await?
            }
        })
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
