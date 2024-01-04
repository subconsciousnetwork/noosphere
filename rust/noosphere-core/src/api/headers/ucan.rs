use crate::{authority::SUPPORTED_KEYS, data::Jwt};
use anyhow::anyhow;
use cid::Cid;
use headers::{self, Header, HeaderName, HeaderValue};
use once_cell::sync::Lazy;
use ucan::{chain::ProofChain, crypto::did::DidParser, store::UcanJwtStore};

static UCAN_NAME: Lazy<HeaderName> = Lazy::new(|| HeaderName::from_static("ucan"));

/// A typed header for the `ucan` header, a tuple of [Cid] and [Jwt],
/// adhering to the [UCAN as Bearer Token](https://github.com/ucan-wg/ucan-http-bearer-token)
/// specification.
///
/// TODO(#708): Note that in the 0.3.0 spec, a single `ucans` header is used.
/// This implementation is based on an earlier version with multiple `ucan`
/// headers.
///
/// The values are **not** validated during parsing beyond legitimate
/// looking [Cid]s and [Jwt]s here. Use [Ucan::as_proof_chain] to
/// validate and construct a [ProofChain].
pub struct Ucan(Vec<(Cid, Jwt)>);

impl Header for Ucan {
    fn name() -> &'static HeaderName {
        &UCAN_NAME
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        I: Iterator<Item = &'i HeaderValue>,
    {
        let mut ucans = vec![];
        for header in values.by_ref() {
            let value = header.to_str().map_err(|_| headers::Error::invalid())?;
            let mut parts: Vec<&str> = value.split_ascii_whitespace().take(2).collect();

            let jwt: Jwt = parts
                .pop()
                .ok_or_else(headers::Error::invalid)?
                .to_string()
                .into();
            let cid: Cid = parts
                .pop()
                .ok_or_else(headers::Error::invalid)?
                .try_into()
                .map_err(|_| headers::Error::invalid())?;

            ucans.push((cid, jwt));
        }

        Ok(Ucan(ucans))
    }

    fn encode<E>(&self, values: &mut E)
    where
        E: Extend<HeaderValue>,
    {
        for (cid, jwt) in self.0.iter() {
            let value = HeaderValue::from_str(&format!("{} {}", cid, jwt)).unwrap();
            values.extend(std::iter::once(value));
        }
    }
}

impl Ucan {
    /// Construct a [ProofChain] from the collected `ucan` header
    /// values, validating the cryptographic integrity and time bounds
    /// of the UCANs. Capabilities can then be assessed using the [ProofChain].
    pub async fn as_proof_chain(
        &self,
        bearer: &headers::authorization::Bearer,
        mut db: impl UcanJwtStore,
    ) -> anyhow::Result<ProofChain> {
        for (cid, jwt) in self.0.iter() {
            // TODO(#261): We need a worker process that purges garbage UCANs
            let actual_cid = db.write_token(jwt.into()).await?;
            if actual_cid != *cid {
                return Err(anyhow!("Cid and Jwt do not match."));
            }
        }

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let proof_chain =
            ProofChain::try_from_token_string(bearer.token(), None, &mut did_parser, &db).await?;

        let ucan = proof_chain.ucan();
        trace!("Validating authority for {:#?}", ucan);
        ucan.validate(None, &mut did_parser).await?;

        Ok(proof_chain)
    }
}

impl From<Ucan> for Vec<(Cid, Jwt)> {
    fn from(value: Ucan) -> Self {
        value.0
    }
}
