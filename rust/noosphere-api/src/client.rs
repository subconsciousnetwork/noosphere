use crate::{
    authority::{GatewayAction, GatewayReference},
    data::{FetchParameters, FetchResponse, IdentifyResponse, PushBody, PushResponse},
    url::{GatewayIdentity, GatewayRequestUrl, Route},
};

use anyhow::{anyhow, Result};
use noosphere_cbor::TryDagCbor;
use reqwest::Body;
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
    ucan::Ucan,
};
use url::Url;

pub struct Client<'a, Credential: KeyMaterial> {
    gateway_identity: GatewayIdentity,
    proofs: Option<Vec<Ucan>>,
    credential: &'a Credential,
    client: reqwest::Client,
}

impl<'a, Credential: KeyMaterial> Client<'a, Credential> {
    pub async fn identify(
        api_base: &str,
        credential: &'a Credential,
        proofs: Option<Vec<Ucan>>,
        expected_identity: Option<&str>,
    ) -> Result<Client<'a, Credential>> {
        let mut url = Url::parse(api_base)?;

        let scheme = url.scheme().to_string();
        let host = url
            .host()
            .ok_or_else(|| anyhow!("No domain specified in {}", api_base))?
            .to_string();
        let port = url.port().unwrap_or(80);

        url.set_path(&Route::Identify.to_string());

        let IdentifyResponse { identity } = reqwest::get(url).await?.json().await?;

        if let Some(expected_identity) = expected_identity {
            if identity != expected_identity {
                return Err(anyhow!(
                    "Expected gateway {} but got {}",
                    expected_identity,
                    identity
                ));
            }
        }

        let gateway_identity = GatewayIdentity {
            scheme,
            host,
            port,
            did: identity,
        };

        Ok(Client {
            gateway_identity,
            proofs,
            credential,
            client: reqwest::Client::new(),
        })
    }

    pub fn gateway_identity(&self) -> &GatewayIdentity {
        &self.gateway_identity
    }

    async fn make_bearer_token(
        &self,
        capability: &Capability<GatewayReference, GatewayAction>,
    ) -> Result<String> {
        let mut builder = UcanBuilder::default()
            .issued_by(self.credential)
            .for_audience(&self.gateway_identity.did)
            .with_lifetime(120)
            .claiming_capability(capability)
            .with_nonce();

        if let Some(proofs) = &self.proofs {
            for proof in proofs {
                builder = builder.witnessed_by(proof);
            }
        }

        Ok(builder.build()?.sign().await?.encode()?)
    }

    async fn fetch(&self, params: &FetchParameters) -> Result<FetchResponse> {
        let url = Url::try_from(GatewayRequestUrl(
            &self.gateway_identity,
            Route::Fetch,
            Some(params),
        ))?;
        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(GatewayReference {
                    did: self.gateway_identity.did.clone(),
                }),
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
        let url = Url::try_from(GatewayRequestUrl::<()>(
            &self.gateway_identity,
            Route::Push,
            None,
        ))?;

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(GatewayReference {
                    did: self.gateway_identity.did.clone(),
                }),
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
