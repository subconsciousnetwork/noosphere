use std::{str::FromStr};

use crate::{
    authority::{GatewayAction, GatewayIdentity},
    data::{FetchParameters, FetchResponse, IdentifyResponse, PushBody, PushResponse},
    gateway::{GatewayReference, GatewayRequestUrl, Route},
};

use anyhow::Result;
use cid::Cid;
use noosphere_cbor::TryDagCbor;
use reqwest::{header::HeaderMap, Body};
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
    store::{UcanJwtStore, UcanStore},
    ucan::Ucan,
};
use url::Url;

pub struct Client<'a, K, S>
where
    K: KeyMaterial,
    S: UcanStore,
{
    pub gateway: GatewayReference,
    pub credential: &'a K,
    pub authorization: Vec<Ucan>,
    pub store: S,
    client: reqwest::Client,
}

impl<'a, K, S> Client<'a, K, S>
where
    K: KeyMaterial,
    S: UcanStore,
{
    pub async fn identify(
        gateway: &GatewayReference,
        credential: &'a K,
        authorization: Option<Vec<Ucan>>,
        store: S,
    ) -> Result<Client<'a, K, S>> {
        let mut url = Url::try_from(gateway)?;

        url.set_path(&Route::Identify.to_string());

        let IdentifyResponse { identity } = reqwest::get(url).await?.json().await?;
        let claimed_identity = GatewayIdentity { did: identity };

        let mut gateway = gateway.clone();
        gateway.ensure_identity(&claimed_identity)?;

        Ok(Client {
            gateway,
            credential,
            authorization: authorization.unwrap_or_default(),
            store,
            client: reqwest::Client::new(),
        })
    }

    async fn make_bearer_token(
        &self,
        capability: &Capability<GatewayIdentity, GatewayAction>,
    ) -> Result<(String, HeaderMap)> {
        let mut builder = UcanBuilder::default()
            .issued_by(self.credential)
            .for_audience(&self.gateway.require_identity()?.did)
            .with_lifetime(120)
            .claiming_capability(capability)
            .with_nonce();

        for proof in &self.authorization {
            builder = builder.witnessed_by(proof);
        }

        let final_ucan = builder.build()?.sign().await?;

        // TODO: We should integrate a helper for this kind of stuff into rs-ucan
        let mut proofs_to_search: Vec<String> = final_ucan.proofs().clone();
        let mut ucan_headers = HeaderMap::new();

        println!("Making bearer token... {:?}", proofs_to_search);
        while let Some(cid_string) = proofs_to_search.pop() {
            let cid = Cid::from_str(cid_string.as_str())?;
            let jwt = self.store.require_token(&cid).await?;
            let ucan = Ucan::try_from_token_string(&jwt)?;

            println!("Adding UCAN header for {}", cid);

            proofs_to_search.extend(ucan.proofs().clone().into_iter());
            ucan_headers.append("ucan", format!("{} {}", cid, jwt).parse()?);
        }

        // TODO: It is inefficient to send the same UCANs with every request,
        // we should probably establish a conventional flow for syncing UCANs
        // this way only once when pairing a gateway. For now, this is about the
        // same efficiency as what we had before when UCANs were all inlined to
        // a single token.
        Ok((final_ucan.encode()?, ucan_headers))
    }

    async fn fetch(&self, params: &FetchParameters) -> Result<FetchResponse> {
        let url = Url::try_from(GatewayRequestUrl(&self.gateway, Route::Fetch, Some(params)))?;
        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(self.gateway.require_identity()?.clone()),
            },
            can: GatewayAction::Fetch,
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
        let url = Url::try_from(GatewayRequestUrl::<()>(&self.gateway, Route::Push, None))?;

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(self.gateway.require_identity()?.clone()),
            },
            can: GatewayAction::Push,
        };

        let (token, ucan_headers) = self.make_bearer_token(&capability).await?;

        Ok(self
            .client
            .put(url)
            .bearer_auth(token)
            .headers(ucan_headers)
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(push_body.try_into_dag_cbor()?))
            .send()
            .await?
            .json()
            .await?)
    }
}
