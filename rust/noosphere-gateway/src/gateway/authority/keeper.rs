use anyhow::{anyhow, Result};
use async_std::sync::Mutex;
use async_trait::async_trait;
use axum::{
    extract::{FromRequest, RequestParts},
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use cid::Cid;
use noosphere_storage::ucan::UcanStore;
use noosphere_storage::{interface::BlockStore, native::NativeStore};
use serde_bytes::Bytes;
use std::ops::{Deref};
use std::sync::Arc;
use ucan::{
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::did::DidParser,
};

use crate::gateway::{
    environment::{Blocks, GatewayConfig},
    AuthzError,
};

use noosphere_api::authority::{GatewayAction, GatewayIdentity, GATEWAY_SEMANTICS};

pub struct GatewayAuthority {
    proof_chain: ProofChain,
    config: Arc<GatewayConfig>,
}

impl GatewayAuthority {
    pub async fn for_bearer(
        config: Arc<GatewayConfig>,
        did_parser: Arc<Mutex<DidParser>>,
        blocks: Blocks<NativeStore>,
        auth_token: &str,
    ) -> Result<GatewayAuthority> {
        let mut did_parser = did_parser.lock().await;
        let proof_chain = ProofChain::try_from_token_string(
            auth_token,
            &mut did_parser,
            &UcanStore(blocks.deref().clone()),
        )
        .await?;
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
                kind: Resource::Scoped(GatewayIdentity {
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

        let mut blocks = req
            .extensions()
            .get::<Blocks<NativeStore>>()
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

        // TODO: We should write a typed header thing for this:
        let ucan_headers = req.headers().get_all("ucan");
        for header in ucan_headers.iter() {
            let value = header.to_str().map_err(|_| AuthzError::MalformedToken)?;
            let mut parts: Vec<&str> = value.split_ascii_whitespace().take(2).collect();

            let jwt = parts.pop().ok_or(AuthzError::MalformedToken)?;
            let cid_string = parts.pop().ok_or(AuthzError::MalformedToken)?;

            let cid = Cid::try_from(cid_string).map_err(|_| AuthzError::MalformedToken)?;

            println!("SAVING {} -> {}", cid, jwt);

            blocks
                .put_block(&cid, Bytes::new(jwt.as_bytes()))
                .await
                .map_err(|_| AuthzError::Internal("Failed to save UCAN JWT".into()))?;
        }

        Ok(GatewayAuthority::for_bearer(
            config.clone(),
            did_parser.clone(),
            blocks.clone(),
            bearer.token(),
        )
        .await
        .map_err(|error| {
            error!("{:?}", error);
            AuthzError::InvalidCredentials
        })?)
    }
}
