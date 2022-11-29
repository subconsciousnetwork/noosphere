use std::str::FromStr;

use crate::{
    data::{FetchParameters, FetchResponse, IdentifyResponse, PushBody, PushResponse},
    route::{Route, RouteUrl},
};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;

use noosphere_core::authority::{Author, SphereAction, SphereReference};
use noosphere_storage::{block_deserialize, block_serialize};
use reqwest::{header::HeaderMap, Body, StatusCode};
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
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
            &Capability {
                with: With::Resource {
                    kind: Resource::Scoped(SphereReference {
                        did: sphere_identity.to_string(),
                    }),
                },
                can: SphereAction::Fetch,
            },
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
        capability: &Capability<SphereReference, SphereAction>,
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

        match authorization.resolve_ucan(store).await {
            Ok(ucan) => {
                // TODO(ucan-wg/rs-ucan#37): We should integrate a helper for this kind of stuff into rs-ucan
                let mut proofs_to_search: Vec<String> = ucan.proofs().clone();

                debug!("Making bearer token... {:?}", proofs_to_search);
                while let Some(cid_string) = proofs_to_search.pop() {
                    let cid = Cid::from_str(cid_string.as_str())?;
                    let jwt = store.require_token(&cid).await?;
                    let ucan = Ucan::try_from_token_string(&jwt)?;

                    debug!("Adding UCAN header for {}", cid);

                    proofs_to_search.extend(ucan.proofs().clone().into_iter());
                    ucan_headers.append("ucan", format!("{} {}", cid, jwt).parse()?);
                }

                ucan_headers.append(
                    "ucan",
                    format!("{} {}", authorization_cid, ucan.encode()?).parse()?,
                );
            }
            _ => {
                warn!("Unable to resolve authorization to a UCAN; it will be used as a blind proof")
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

    pub async fn fetch(&self, params: &FetchParameters) -> Result<FetchResponse> {
        let url = Url::try_from(RouteUrl(&self.api_base, Route::Fetch, Some(params)))?;
        debug!("Client fetching blocks from {}", url);
        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: self.sphere_identity.clone(),
                }),
            },
            can: SphereAction::Fetch,
        };

        let (token, ucan_headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        let bytes = self
            .client
            .get(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .send()
            .await?
            .bytes()
            .await?;

        block_deserialize::<DagCborCodec, _>(&bytes)
    }

    pub async fn push(&self, push_body: &PushBody) -> Result<PushResponse> {
        let url = Url::try_from(RouteUrl::<()>(&self.api_base, Route::Push, None))?;
        debug!(
            "Client pushing {} blocks for sphere {} to {}",
            push_body.blocks.len(),
            push_body.sphere,
            url
        );
        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: self.sphere_identity.clone(),
                }),
            },
            can: SphereAction::Push,
        };

        let (token, ucan_headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        let (_, push_body_bytes) = block_serialize::<DagCborCodec, _>(push_body)?;

        let bytes = self
            .client
            .put(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(push_body_bytes))
            .send()
            .await?
            .bytes()
            .await?;

        block_deserialize::<DagCborCodec, _>(bytes.as_ref())
    }
}
