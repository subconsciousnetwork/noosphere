use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    headers::{authorization::Bearer, Authorization},
    http::{request::Parts, StatusCode},
    TypedHeader,
};
use libipld_core::cid::Cid;
use noosphere_core::authority::{SphereAbility, SphereReference, SPHERE_SEMANTICS};
use noosphere_core::context::SphereContext;
use noosphere_storage::Storage;
use tokio::sync::Mutex;
use ucan::{capability::CapabilityView, chain::ProofChain, store::UcanJwtStore};

use super::GatewayScope;

/// This is a construct that can be generated on a per-request basis and
/// embodies the authorization status of the request-maker as it is
/// represented by a UCAN. Any request handler can use a GatewayAuthority
/// to test if a required capability is satisfied by the authorization
/// presented by the maker of the request.
pub struct GatewayAuthority<S> {
    proof: ProofChain,
    scope: GatewayScope,
    marker: std::marker::PhantomData<S>,
}

impl<S> GatewayAuthority<S>
where
    S: Storage + 'static,
{
    pub fn try_authorize(
        &self,
        capability: &CapabilityView<SphereReference, SphereAbility>,
    ) -> Result<(), StatusCode> {
        let capability_infos = self.proof.reduce_capabilities(&SPHERE_SEMANTICS);

        for capability_info in capability_infos {
            trace!("Checking capability: {:?}", capability_info.capability);
            if capability_info
                .originators
                .contains(self.scope.counterpart.as_str())
                && capability_info.capability.enables(capability)
            {
                debug!("Authorized!");
                return Ok(());
            }
        }

        Err(StatusCode::UNAUTHORIZED)
    }
}

#[async_trait]
impl<S, State> FromRequestParts<State> for GatewayAuthority<S>
where
    State: Send + Sync,
    S: Storage + 'static,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &State) -> Result<Self, Self::Rejection> {
        let sphere_context = parts
            .extensions
            .get::<Arc<Mutex<SphereContext<S>>>>()
            .ok_or_else(|| {
                error!("Could not find DidParser in extensions");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .clone();

        // Get the scope of this gateway
        let gateway_scope = parts
            .extensions
            .get::<GatewayScope>()
            .ok_or_else(|| {
                error!("Could not find GatewayScope in extensions");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .clone();

        // Extract the bearer token
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|error| {
                    error!("{:?}", error);
                    StatusCode::BAD_REQUEST
                })?;

        let mut db = {
            let sphere_context = sphere_context.lock().await;
            sphere_context.db().clone()
        };

        let ucan_headers = parts.headers.get_all("ucan").into_iter();

        // TODO: We should write a typed header thing for this:
        for header in ucan_headers {
            let value = header.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
            let mut parts: Vec<&str> = value.split_ascii_whitespace().take(2).collect();

            let jwt = parts.pop().ok_or(StatusCode::BAD_REQUEST)?;

            let cid_string = parts.pop().ok_or(StatusCode::BAD_REQUEST)?;
            let claimed_cid = Cid::try_from(cid_string).map_err(|_| StatusCode::BAD_REQUEST)?;

            // TODO(#261): We need a worker process that purges garbage UCANs
            let actual_cid = db
                .write_token(jwt)
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?;

            if claimed_cid != actual_cid {
                return Err(StatusCode::BAD_REQUEST);
            }
        }

        let proof_chain = {
            let mut sphere_context = sphere_context.lock().await;
            let did_parser = sphere_context.did_parser_mut();
            let proof_chain =
                ProofChain::try_from_token_string(bearer.token(), None, did_parser, &db)
                    .await
                    .map_err(|error| {
                        error!("{:?}", error);
                        StatusCode::BAD_REQUEST
                    })?;

            proof_chain
                .ucan()
                .validate(None, did_parser)
                .await
                .map_err(|error| {
                    error!("{:?}", error);
                    StatusCode::UNAUTHORIZED
                })?;

            proof_chain
        };

        Ok(GatewayAuthority {
            scope: gateway_scope.clone(),
            proof: proof_chain,
            marker: std::marker::PhantomData,
        })
    }
}
