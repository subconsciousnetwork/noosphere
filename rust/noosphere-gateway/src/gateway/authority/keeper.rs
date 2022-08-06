use anyhow::{anyhow, Result};
use async_std::sync::Mutex;
use async_trait::async_trait;
use axum::{
    extract::{FromRequest, RequestParts},
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use std::sync::Arc;
use ucan::{
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::did::DidParser,
};

use crate::gateway::{environment::GatewayConfig, AuthzError};

use noosphere_api::authority::{GatewayAction, GatewayReference, GATEWAY_SEMANTICS};

pub struct GatewayAuthority {
    proof_chain: ProofChain,
    config: Arc<GatewayConfig>,
}

impl GatewayAuthority {
    pub async fn for_bearer(
        config: Arc<GatewayConfig>,
        did_parser: Arc<Mutex<DidParser>>,
        auth_token: &str,
    ) -> Result<GatewayAuthority> {
        let mut did_parser = did_parser.lock().await;
        let proof_chain = ProofChain::try_from_token_string(auth_token, &mut did_parser).await?;
        Ok(GatewayAuthority {
            config,
            proof_chain,
        })
    }

    pub async fn try_authorize(&self, action: GatewayAction) -> Result<()> {
        let owner_did = self.config.expect_owner_did().await?;
        let gateway_identity = self.config.expect_identity().await?;

        if self.proof_chain.ucan().audience() != gateway_identity {
            return Err(anyhow!(AuthzError::WrongCredentials));
        }

        let desired_capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(GatewayReference {
                    did: gateway_identity,
                }),
            },
            can: action,
        };

        let capability_infos = self.proof_chain.reduce_capabilities(&GATEWAY_SEMANTICS);

        for info in capability_infos {
            if info.capability.enables(&desired_capability) && info.originators.contains(&owner_did)
            {
                return Ok(());
            }
        }

        Err(anyhow!(AuthzError::WrongCredentials))
    }
}

#[async_trait]
impl<B> FromRequest<B> for GatewayAuthority
where
    B: Send,
{
    type Rejection = AuthzError;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let did_parser = req
            .extensions()
            .get::<Arc<Mutex<DidParser>>>()
            .ok_or(AuthzError::Internal("No DID parser found".into()))?
            .clone();

        let config = req
            .extensions()
            .get::<Arc<GatewayConfig>>()
            .ok_or(AuthzError::Internal(
                "No gateway configuration found".into(),
            ))?
            .clone();

        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|error| {
                    error!("{:?}", error);
                    AuthzError::MalformedToken
                })?;

        Ok(
            GatewayAuthority::for_bearer(config.clone(), did_parser.clone(), bearer.token())
                .await
                .map_err(|error| {
                    error!("{:?}", error);
                    AuthzError::InvalidCredentials
                })?,
        )
    }
}
