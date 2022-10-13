use std::str::FromStr;

use crate::{
    data::{FetchParameters, FetchResponse, IdentifyResponse, PushBody, PushResponse},
    route::{Route, RouteUrl},
};

use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere::authority::{SphereAction, SphereReference};
use noosphere_cbor::TryDagCbor;
use noosphere_storage::encoding::block_serialize;
use reqwest::{header::HeaderMap, Body};
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    crypto::{did::DidParser, KeyMaterial},
    store::{UcanJwtStore, UcanStore},
    ucan::Ucan,
};
use url::Url;

pub struct Client<'a, K, S>
where
    K: KeyMaterial,
    S: UcanStore,
{
    pub session: IdentifyResponse,
    pub sphere_identity: String,
    pub api_base: Url,
    pub credential: &'a K,
    pub authorization: Ucan,
    pub store: S,
    client: reqwest::Client,
}

impl<'a, K, S> Client<'a, K, S>
where
    K: KeyMaterial,
    S: UcanStore,
{
    pub async fn identify(
        sphere_identity: &str,
        api_base: &Url,
        credential: &'a K,
        authorization: Ucan,
        did_parser: &mut DidParser,
        store: S,
    ) -> Result<Client<'a, K, S>> {
        debug!("Initializing Noosphere API client");
        debug!("Client represents sphere {}", sphere_identity);
        debug!("Client targetting API at {}", api_base);

        let mut url = api_base.clone();

        url.set_path(&Route::Identify.to_string());

        let client = reqwest::Client::new();

        let identify_response: IdentifyResponse = client
            .get(url)
            .bearer_auth(authorization.encode()?)
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
            credential,
            authorization,
            store,
            client,
        })
    }

    async fn make_bearer_token(
        &self,
        capability: &Capability<SphereReference, SphereAction>,
    ) -> Result<(String, HeaderMap)> {
        let ucan = UcanBuilder::default()
            .issued_by(self.credential)
            .for_audience(&self.session.sphere_identity)
            .with_lifetime(120)
            .claiming_capability(capability)
            .witnessed_by(&self.authorization)
            .with_nonce()
            .build()?
            .sign()
            .await?;

        // TODO: We should integrate a helper for this kind of stuff into rs-ucan
        let mut proofs_to_search: Vec<String> = ucan.proofs().clone();
        let mut ucan_headers = HeaderMap::new();

        debug!("Making bearer token... {:?}", proofs_to_search);
        while let Some(cid_string) = proofs_to_search.pop() {
            let cid = Cid::from_str(cid_string.as_str())?;
            let jwt = self.store.require_token(&cid).await?;
            let ucan = Ucan::try_from_token_string(&jwt)?;

            debug!("Adding UCAN header for {}", cid);

            proofs_to_search.extend(ucan.proofs().clone().into_iter());
            ucan_headers.append("ucan", format!("{} {}", cid, jwt).parse()?);
        }

        // TODO: It is inefficient to send the same UCANs with every request,
        // we should probably establish a conventional flow for syncing UCANs
        // this way only once when pairing a gateway. For now, this is about the
        // same efficiency as what we had before when UCANs were all inlined to
        // a single token.
        Ok((ucan.encode()?, ucan_headers))
    }

    async fn fetch(&self, params: &FetchParameters) -> Result<FetchResponse> {
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

        let (token, ucan_headers) = self.make_bearer_token(&capability).await?;

        let bytes = self
            .client
            .get(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .send()
            .await?
            .bytes()
            .await?;

        FetchResponse::try_from_dag_cbor(&bytes)
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

        let (token, ucan_headers) = self.make_bearer_token(&capability).await?;

        let (_, push_body_bytes) = block_serialize::<DagCborCodec, _>(push_body)?;

        Ok(self
            .client
            .put(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(push_body_bytes))
            .send()
            .await?
            .json()
            .await?)
    }
}
