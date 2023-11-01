use crate::api::route::RouteSignature;
use crate::route_display;

use super::ReplicationMode;

/// The version of the API represented by this module
pub const API_VERSION: &str = "v0alpha1";

/// An enum whose variants represent all of the routes in this version of the API
pub enum Route {
    /// Fetch the latest canonical history of a sphere from the gateway
    Fetch,
    /// Push the latest local history of a sphere from a client to the gateway
    Push,
    /// Get the DID of the gateway
    Did,
    /// Get a signed verification of the gateway's credentials
    Identify,
    /// Replicate content from the broader Noosphere network
    Replicate(Option<ReplicationMode>),
}

route_display!(Route);

impl RouteSignature for Route {
    fn to_fragment(&self) -> String {
        match self {
            Route::Fetch => "fetch".into(),
            Route::Push => "push".into(),
            Route::Did => "did".into(),
            Route::Identify => "identify".into(),
            Route::Replicate(mode) => match mode {
                Some(mode) => format!("replicate/{mode}"),
                None => "replicate/:link_or_did".into(),
            },
        }
    }

    fn api_version(&self) -> &str {
        API_VERSION
    }
}
