use std::{ops::Deref, sync::Arc};

use crate::{
    authority::{GatewayAction, GatewayIdentity},
    data::{FetchParameters, FetchResponse, IdentifyResponse, PushBody, PushResponse},
    gateway::{GatewayReference, GatewayRequestUrl, Route},
};

use anyhow::Result;
use noosphere_cbor::TryDagCbor;
use noosphere_storage::interface::{StorageProvider, Store};
use reqwest::Body;
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
    ucan::Ucan,
};
use url::Url;

pub struct Client<'a, K>
where
    K: KeyMaterial,
{
    pub gateway: GatewayReference,
    pub credential: &'a K,
    pub authorization: Vec<Ucan>,
    client: reqwest::Client,
}

impl<'a, K> Client<'a, K>
where
    K: KeyMaterial,
{
    pub async fn identify(
        gateway: &GatewayReference,
        credential: &'a K,
        authorization: Option<Vec<Ucan>>,
    ) -> Result<Client<'a, K>> {
        let mut url = Url::try_from(gateway)?;

        url.set_path(&Route::Identify.to_string());

        let IdentifyResponse { identity } = reqwest::get(url).await?.json().await?;
        let claimed_identity = GatewayIdentity { did: identity };

        let mut gateway = gateway.clone();
        gateway.ensure_identity(&claimed_identity)?;

        Ok(Client {
            gateway,
            credential,
            authorization: authorization.unwrap_or_else(|| Vec::new()),
            client: reqwest::Client::new(),
        })
    }

    async fn make_bearer_token(
        &self,
        capability: &Capability<GatewayIdentity, GatewayAction>,
    ) -> Result<String> {
        let mut builder = UcanBuilder::default()
            .issued_by(self.credential)
            .for_audience(&self.gateway.require_identity()?.did)
            .with_lifetime(120)
            .claiming_capability(capability)
            .with_nonce();

        for proof in &self.authorization {
            builder = builder.witnessed_by(proof);
        }

        Ok(builder.build()?.sign().await?.encode()?)
    }

    async fn fetch(&self, params: &FetchParameters) -> Result<FetchResponse> {
        let url = Url::try_from(GatewayRequestUrl(&self.gateway, Route::Fetch, Some(params)))?;
        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(self.gateway.require_identity()?.clone()),
            },
            can: GatewayAction::Fetch,
        };

        let token = self.make_bearer_token(&capability).await?;

        let bytes = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await?
            .bytes()
            .await?;

        Ok(FetchResponse::try_from_dag_cbor(&bytes)?)
    }

    pub async fn push(&self, push_body: &PushBody) -> Result<PushResponse> {
        let url = Url::try_from(GatewayRequestUrl::<()>(&self.gateway, Route::Push, None))?;

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(self.gateway.require_identity()?.clone()),
            },
            can: GatewayAction::Push,
        };

        let token = self.make_bearer_token(&capability).await?;

        Ok(self
            .client
            .put(url)
            .bearer_auth(token)
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(push_body.try_into_dag_cbor()?))
            .send()
            .await?
            .json()
            .await?)
    }
}
