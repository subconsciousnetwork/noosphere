use anyhow::{anyhow, Result};
use std::fmt::Display;
use url::{ParseError, Url};

use crate::data::AsQuery;

pub const API_VERSION: &str = "v0alpha1";

pub enum Route {
    Fetch,
    Push,
    Publish,
    Identify,
}

impl Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fragment = match self {
            Route::Fetch => "fetch",
            Route::Push => "push",
            Route::Publish => "publish",
            Route::Identify => "identify",
        };

        write!(f, "/api/{}/{}", API_VERSION, fragment)
    }
}

#[derive(Clone)]
pub struct GatewayIdentity {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub did: String,
}

impl GatewayIdentity {
    pub fn try_from_uri_and_did(uri: &str, did: &str) -> Result<GatewayIdentity> {
        let url = Url::parse(uri)?;
        Ok(GatewayIdentity {
            scheme: url.scheme().to_string(),
            host: url
                .domain()
                .ok_or_else(|| anyhow!("Could not derive domain from {}", uri))?
                .to_string(),
            port: url
                .port()
                .ok_or_else(|| anyhow!("Could not derive port from {}", uri))?,
            did: did.to_string(),
        })
    }

    pub fn try_as_base_url(&self) -> Result<Url> {
        Url::parse(&format!("{}://{}:{}", self.scheme, self.host, self.port))
            .map_err(|error| anyhow!(error))
    }
}

impl TryFrom<&GatewayIdentity> for Url {
    type Error = url::ParseError;
    fn try_from(origin: &GatewayIdentity) -> Result<Self, Self::Error> {
        Url::parse(&format!(
            "{}://{}:{}",
            origin.scheme, origin.host, origin.port
        ))
    }
}

pub struct GatewayRequestUrl<'a, 'b, Params: AsQuery = ()>(
    pub &'a GatewayIdentity,
    pub Route,
    pub Option<&'b Params>,
);

impl<'a, 'b, Params: AsQuery> TryFrom<GatewayRequestUrl<'a, 'b, Params>> for Url {
    type Error = ParseError;

    fn try_from(value: GatewayRequestUrl<'a, 'b, Params>) -> Result<Self, Self::Error> {
        let GatewayRequestUrl(
            GatewayIdentity {
                scheme,
                host: domain,
                port,
                ..
            },
            route,
            params,
        ) = value;
        let mut url = Url::parse(&format!("{}://{}:{}", scheme, domain, port))?;
        url.set_path(&route.to_string());
        if let Some(params) = params {
            url.set_query(params.as_query().as_deref());
        } else {
            url.set_query(None);
        }
        Ok(url)
    }
}
