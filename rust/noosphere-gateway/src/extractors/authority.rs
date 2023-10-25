use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    headers::{authorization::Bearer, Authorization},
    http::{request::Parts, StatusCode},
    TypedHeader,
};
use noosphere_core::{
    api::headers::{self as noosphere_headers},
    authority::{SphereAbility, SphereReference, SPHERE_SEMANTICS},
    context::HasMutableSphereContext,
    data::Did,
};
use noosphere_storage::Storage;
use ucan::capability::CapabilityView;

use crate::extractors::map_bad_request;

/// Represents the scope of a gateway request's authorization and sphere
/// access.
///
/// Embodies the authorization status of the request-maker as it is
/// represented by its `ucan` headers. Any request handler can use a [GatewayAuthority]
/// to test if a required capability is satisfied by the authorization
/// presented by the maker of the request.
pub struct GatewayAuthority {
    bearer: Bearer,
    ucans: noosphere_headers::Ucan,
}

impl GatewayAuthority {
    pub async fn try_authorize<C, S>(
        &self,
        sphere_context: &mut C,
        counterpart: &Did,
        capability: &CapabilityView<SphereReference, SphereAbility>,
    ) -> Result<(), StatusCode>
    where
        C: HasMutableSphereContext<S>,
        S: Storage + 'static,
    {
        let db = {
            let sphere_context: C::SphereContext = sphere_context
                .sphere_context()
                .await
                .map_err(map_bad_request)?;
            sphere_context.db().clone()
        };

        let proof_chain = self
            .ucans
            .as_proof_chain(&self.bearer, db)
            .await
            .map_err(map_bad_request)?;

        let capability_infos = proof_chain.reduce_capabilities(&SPHERE_SEMANTICS);

        for capability_info in capability_infos {
            trace!("Checking capability: {:?}", capability_info.capability);
            if capability_info.originators.contains(counterpart.as_str())
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
impl<State> FromRequestParts<State> for GatewayAuthority
where
    State: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &State) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(map_bad_request)?;

        let ucans: noosphere_headers::Ucan =
            TypedHeader::<noosphere_headers::Ucan>::from_request_parts(parts, state)
                .await
                .map_err(map_bad_request)?
                .0;

        Ok(GatewayAuthority { bearer, ucans })
    }
}
