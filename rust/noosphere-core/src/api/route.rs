use anyhow::Result;
use std::fmt::Display;

use url::Url;

use crate::api::data::AsQuery;

/// A helper macro to quickly implement a common [Display] format for
/// Noosphere Gateway REST API routes
#[macro_export]
macro_rules! route_display {
    ($routes:ty) => {
        impl std::fmt::Display for $routes {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "/api/{}/{}", self.api_version(), self.to_fragment())
            }
        }
    };
}

/// A helper trait implemented by any route enum that enables it to be easily
/// serialized as part of a URL
pub trait RouteSignature: Display {
    /// Produces the path fragment for a given route
    fn to_fragment(&self) -> String;
    /// Gets the API version for the given route
    fn api_version(&self) -> &str;
}

/// The [RouteUrl] is a helper to produce a [Url] from any implementor of [RouteSignature], but specifically
/// the enums found in the [crate::v0alpha1] and [crate::v0alpha2] modules.
pub struct RouteUrl<'a, 'b, Route: RouteSignature, Params: AsQuery = ()>(
    pub &'a Url,
    pub Route,
    pub Option<&'b Params>,
);

impl<'a, 'b, Route: RouteSignature, Params: AsQuery> TryFrom<RouteUrl<'a, 'b, Route, Params>>
    for Url
{
    type Error = anyhow::Error;

    fn try_from(value: RouteUrl<'a, 'b, Route, Params>) -> Result<Self, Self::Error> {
        let RouteUrl(api_base, route, params) = value;
        let mut url = api_base.clone();
        url.set_path(&route.to_string());
        if let Some(params) = params {
            url.set_query(params.as_query()?.as_deref());
        } else {
            url.set_query(None);
        }
        Ok(url)
    }
}
