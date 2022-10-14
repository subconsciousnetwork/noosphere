use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::{FromRequest, RequestParts},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    TypedHeader,
};
use libipld_core::cid::Cid;
use noosphere::authority::{SphereAction, SphereReference, SPHERE_SEMANTICS};
use noosphere_storage::{db::SphereDb, native::NativeStore};

use tokio::sync::Mutex;
use ucan::{
    capability::Capability, chain::ProofChain, crypto::did::DidParser, store::UcanJwtStore,
};

use super::gateway::GatewayScope;

/// This is a construct that can be generated on a per-request basis and
/// embodies the authorization status of the request-maker as it is
/// represented by a UCAN. Any request handler can use a GatewayAuthority
/// to test if a required capability is satisfied by the authorization
/// presented by the maker of the request.
pub struct GatewayAuthority {
    proof: ProofChain,
    scope: GatewayScope,
}

impl GatewayAuthority {
    pub fn expect_audience(&self, audience: &str) -> Result<(), StatusCode> {
        if self.proof.ucan().audience() != audience {
            return Err(StatusCode::UNAUTHORIZED);
        }

        Ok(())
    }

    pub fn try_authorize(
        &self,
        capability: &Capability<SphereReference, SphereAction>,
    ) -> Result<(), StatusCode> {
        let capability_infos = self.proof.reduce_capabilities(&SPHERE_SEMANTICS);

        for capability_info in capability_infos {
            trace!("Checking capability: {:?}", capability_info.capability);
            if capability_info
                .originators
                .contains(&self.scope.counterpart)
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
impl<B> FromRequest<B> for GatewayAuthority
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        // Look for the DID parser
        let did_parser = req
            .extensions()
            .get::<Arc<Mutex<DidParser>>>()
            .ok_or_else(|| {
                error!("Could not find DidParser in extensions");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .clone();

        // Look for the SphereDb
        let mut db = req
            .extensions()
            .get::<SphereDb<NativeStore>>()
            .ok_or_else(|| {
                error!("Could not find SphereDb in extensions");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .clone();

        // Get the scope of this gateway
        let gateway_scope = req
            .extensions()
            .get::<GatewayScope>()
            .ok_or_else(|| {
                error!("Could not find GatewayScope in extensions");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .clone();

        // Extract the bearer token
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|error| {
                    error!("{:?}", error);
                    StatusCode::BAD_REQUEST
                })?;

        // TODO: We should write a typed header thing for this:
        let ucan_headers = req.headers().get_all("ucan");
        for header in ucan_headers.iter() {
            let value = header.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
            let mut parts: Vec<&str> = value.split_ascii_whitespace().take(2).collect();

            let jwt = parts.pop().ok_or(StatusCode::BAD_REQUEST)?;

            let cid_string = parts.pop().ok_or(StatusCode::BAD_REQUEST)?;
            let claimed_cid = Cid::try_from(cid_string).map_err(|_| StatusCode::BAD_REQUEST)?;

            // TODO: We need a worker process that purges garbage UCANs
            let actual_cid = db
                .write_token(jwt)
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?;

            if claimed_cid != actual_cid {
                return Err(StatusCode::BAD_REQUEST);
            }
        }

        let mut did_parser = did_parser.lock().await;

        let proof_chain = ProofChain::try_from_token_string(bearer.token(), &mut did_parser, &db)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::BAD_REQUEST
            })?;

        proof_chain
            .ucan()
            .validate(&mut did_parser)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::UNAUTHORIZED
            })?;

        Ok(GatewayAuthority {
            scope: gateway_scope.clone(),
            proof: proof_chain,
        })
    }
}
