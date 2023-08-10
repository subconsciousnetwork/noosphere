use std::str::FromStr;

use crate::{
    data::{
        FetchParameters, IdentifyResponse, PushBody, PushError, PushResponse, ReplicateParameters,
    },
    route::{Route, RouteUrl},
};

use anyhow::{anyhow, Result};
use cid::Cid;
use iroh_car::CarReader;
use libipld_cbor::DagCborCodec;

use noosphere_core::{
    authority::{generate_capability, Author, SphereAbility, SphereReference},
    data::{Link, MemoIpld},
};
use noosphere_storage::{block_deserialize, block_serialize};
use reqwest::{header::HeaderMap, Body, StatusCode};
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::StreamReader;
use ucan::{
    builder::UcanBuilder,
    capability::CapabilityView,
    crypto::{did::DidParser, KeyMaterial},
    store::{UcanJwtStore, UcanStore},
    ucan::Ucan,
};
use url::Url;

/// A [Client] is a simple, portable HTTP client for the Noosphere gateway REST
/// API. It embodies the intended usage of the REST API, which includes an
/// opening handshake (with associated key verification) and various
/// UCAN-authorized verbs over sphere data.
pub struct Client<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: UcanStore,
{
    pub session: IdentifyResponse,
    pub sphere_identity: String,
    pub api_base: Url,
    pub author: Author<K>,
    pub store: S,
    client: reqwest::Client,
}

impl<K, S> Client<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: UcanStore,
{
    pub async fn identify(
        sphere_identity: &str,
        api_base: &Url,
        author: &Author<K>,
        did_parser: &mut DidParser,
        store: S,
    ) -> Result<Client<K, S>> {
        debug!("Initializing Noosphere API client");
        debug!("Client represents sphere {}", sphere_identity);
        debug!("Client targetting API at {}", api_base);

        let client = reqwest::Client::new();

        let mut url = api_base.clone();
        url.set_path(&Route::Did.to_string());

        let did_response = client.get(url).send().await?;

        match did_response.status() {
            StatusCode::OK => (),
            _ => return Err(anyhow!("Unable to look up gateway identity")),
        };

        let gateway_identity = did_response.text().await?;

        let mut url = api_base.clone();
        url.set_path(&Route::Identify.to_string());

        let (jwt, ucan_headers) = Self::make_bearer_token(
            &gateway_identity,
            author,
            &generate_capability(sphere_identity, SphereAbility::Fetch),
            &store,
        )
        .await?;

        let identify_response: IdentifyResponse = client
            .get(url)
            .bearer_auth(jwt)
            .headers(ucan_headers)
            .send()
            .await?
            .json()
            .await?;

        identify_response.verify(did_parser, &store).await?;

        debug!(
            "Handshake succeeded with gateway {}",
            identify_response.gateway_identity
        );

        Ok(Client {
            session: identify_response,
            sphere_identity: sphere_identity.into(),
            api_base: api_base.clone(),
            author: author.clone(),
            store,
            client,
        })
    }

    async fn make_bearer_token(
        gateway_identity: &str,
        author: &Author<K>,
        capability: &CapabilityView<SphereReference, SphereAbility>,
        store: &S,
    ) -> Result<(String, HeaderMap)> {
        let mut signable = UcanBuilder::default()
            .issued_by(&author.key)
            .for_audience(gateway_identity)
            .with_lifetime(120)
            .claiming_capability(capability)
            .with_nonce()
            .build()?;

        let mut ucan_headers = HeaderMap::new();

        let authorization = author.require_authorization()?;
        let authorization_cid = Cid::try_from(authorization)?;

        match authorization.as_ucan(store).await {
            Ok(ucan) => {
                if let Some(ucan_proofs) = ucan.proofs() {
                    // TODO(ucan-wg/rs-ucan#37): We should integrate a helper for this kind of stuff into rs-ucan
                    let mut proofs_to_search: Vec<String> = ucan_proofs.clone();

                    debug!("Making bearer token... {:?}", proofs_to_search);

                    while let Some(cid_string) = proofs_to_search.pop() {
                        let cid = Cid::from_str(cid_string.as_str())?;
                        let jwt = store.require_token(&cid).await?;
                        let ucan = Ucan::from_str(&jwt)?;

                        debug!("Adding UCAN header for {}", cid);

                        if let Some(ucan_proofs) = ucan.proofs() {
                            proofs_to_search.extend(ucan_proofs.clone().into_iter());
                        }

                        ucan_headers.append("ucan", format!("{cid} {jwt}").parse()?);
                    }
                }

                ucan_headers.append(
                    "ucan",
                    format!("{} {}", authorization_cid, ucan.encode()?).parse()?,
                );
            }
            _ => {
                debug!(
                    "Unable to resolve authorization to a UCAN; it will be used as a blind proof"
                )
            }
        };

        // TODO(ucan-wg/rs-ucan#32): This is kind of a hack until we can add proofs by CID
        signable
            .proofs
            .push(Cid::try_from(authorization)?.to_string());

        let jwt = signable.sign().await?.encode()?;

        // TODO: It is inefficient to send the same UCANs with every request,
        // we should probably establish a conventional flow for syncing UCANs
        // this way only once when pairing a gateway. For now, this is about the
        // same efficiency as what we had before when UCANs were all inlined to
        // a single token.
        Ok((jwt, ucan_headers))
    }

    /// Replicate content from Noosphere, streaming its blocks from the
    /// configured gateway. If the gateway doesn't have the desired content, it
    /// will look it up from other sources such as IPFS if they are available.
    /// Note that this means this call can potentially block on upstream
    /// access to an IPFS node (which, depending on the node's network
    /// configuration and peering status, can be quite slow).
    pub async fn replicate(
        &self,
        memo_version: &Cid,
        params: Option<&ReplicateParameters>,
    ) -> Result<impl Stream<Item = Result<(Cid, Vec<u8>)>>> {
        let url = Url::try_from(RouteUrl(
            &self.api_base,
            Route::Replicate(Some(*memo_version)),
            params,
        ))?;

        debug!("Client replicating {} from {}", memo_version, url);

        let capability = generate_capability(&self.sphere_identity, SphereAbility::Fetch);

        let (token, ucan_headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .send()
            .await?;

        Ok(
            CarReader::new(StreamReader::new(response.bytes_stream().map(
                |item| match item {
                    Ok(item) => Ok(item),
                    Err(error) => {
                        error!("Failed to read CAR stream: {}", error);
                        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                    }
                },
            )))
            .await?
            .stream()
            .map(|block| match block {
                Ok(block) => Ok(block),
                Err(error) => Err(anyhow!(error)),
            }),
        )
    }

    pub async fn fetch(
        &self,
        params: &FetchParameters,
    ) -> Result<Option<(Link<MemoIpld>, impl Stream<Item = Result<(Cid, Vec<u8>)>>)>> {
        let url = Url::try_from(RouteUrl(&self.api_base, Route::Fetch, Some(params)))?;

        debug!("Client fetching blocks from {}", url);

        let capability = generate_capability(&self.sphere_identity, SphereAbility::Fetch);
        let (token, ucan_headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .send()
            .await?;

        let reader = CarReader::new(StreamReader::new(response.bytes_stream().map(
            |item| match item {
                Ok(item) => Ok(item),
                Err(error) => {
                    error!("Failed to read CAR stream: {}", error);
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                }
            },
        )))
        .await?;

        let tip = reader.header().roots().first().cloned();

        if let Some(tip) = tip {
            Ok(match tip.codec() {
                // Identity codec = no changes
                0 => None,
                _ => Some((
                    tip.into(),
                    reader.stream().map(|block| match block {
                        Ok(block) => Ok(block),
                        Err(error) => Err(anyhow!(error)),
                    }),
                )),
            })
        } else {
            Ok(None)
        }
    }

    pub async fn push(&self, push_body: &PushBody) -> Result<PushResponse, PushError> {
        let url = Url::try_from(RouteUrl::<()>(&self.api_base, Route::Push, None))?;
        debug!(
            "Client pushing {} blocks for sphere {} to {}",
            push_body.blocks.len(),
            push_body.sphere,
            url
        );
        let capability = generate_capability(&self.sphere_identity, SphereAbility::Push);
        let (token, ucan_headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        let (_, push_body_bytes) = block_serialize::<DagCborCodec, _>(push_body)?;

        let response = self
            .client
            .put(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(push_body_bytes))
            .send()
            .await
            .map_err(|err| PushError::Internal(anyhow!(err)))?;

        if response.status() == StatusCode::CONFLICT {
            return Err(PushError::Conflict);
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|err| PushError::Internal(anyhow!(err)))?;
        Ok(block_deserialize::<DagCborCodec, _>(bytes.as_ref())?)
    }
}
