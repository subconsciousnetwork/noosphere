use crate::{
    extractors::{map_bad_request, GatewayScope},
    GatewayManager,
};
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use noosphere_core::{
    api::headers::{self as noosphere_headers},
    authority::{generate_capability, SphereAbility, SPHERE_SEMANTICS},
    context::HasMutableSphereContext,
};
use noosphere_storage::Storage;
use std::{marker::PhantomData, sync::Arc};

/// Represents the scope of a gateway request's authorization and sphere
/// access.
///
/// Embodies the authorization status of the request-maker as it is
/// represented by its `ucan` headers. Any request handler can use a [GatewayAuthority]
/// to test if a required capability is satisfied by the authorization
/// presented by the maker of the request.
pub struct GatewayAuthority<M, C, S> {
    bearer: Bearer,
    ucans: noosphere_headers::Ucan,
    manager: Arc<M>,
    sphere_context_marker: PhantomData<C>,
    storage_marker: PhantomData<S>,
}

impl<M, C, S> GatewayAuthority<M, C, S>
where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    pub async fn try_authorize(
        &self,
        gateway_scope: &GatewayScope<C, S>,
        required_ability: SphereAbility,
    ) -> Result<C, StatusCode> {
        let counterpart_str = gateway_scope.counterpart.as_str();
        let capability = generate_capability(counterpart_str, required_ability);
        let db = self
            .manager
            .ucan_store()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let proof_chain = self
            .ucans
            .as_proof_chain(&self.bearer, db)
            .await
            .map_err(map_bad_request)?;

        let capability_infos = proof_chain.reduce_capabilities(&SPHERE_SEMANTICS);

        for capability_info in capability_infos {
            trace!("Checking capability: {:?}", capability_info.capability);
            if capability_info.originators.contains(counterpart_str)
                && capability_info.capability.enables(&capability)
            {
                debug!("Authorized!");
                return self
                    .manager
                    .sphere_context(&gateway_scope.counterpart)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        Err(StatusCode::UNAUTHORIZED)
    }
}

#[async_trait]
impl<M, C, S> FromRequestParts<Arc<M>> for GatewayAuthority<M, C, S>
where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<M>,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(map_bad_request)?;

        let ucans: noosphere_headers::Ucan =
            TypedHeader::<noosphere_headers::Ucan>::from_request_parts(parts, state)
                .await
                .map_err(map_bad_request)?
                .0;

        let manager = state.to_owned();
        Ok(GatewayAuthority {
            bearer,
            ucans,
            manager,
            sphere_context_marker: PhantomData,
            storage_marker: PhantomData,
        })
    }
}
