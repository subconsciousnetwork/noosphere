use crate::api::route::RouteSignature;
use crate::route_display;

/// The version of the API represented by this module
pub const API_VERSION: &str = "v0alpha2";

/// An enum whose variants represent all of the routes in this version of the API
pub enum Route {
    /// Push the latest local history of a sphere from a client to the gateway
    Push,
}

route_display!(Route);

impl RouteSignature for Route {
    fn to_fragment(&self) -> String {
        match self {
            Route::Push => "push".to_owned(),
        }
    }

    fn api_version(&self) -> &str {
        API_VERSION
    }
}
