use anyhow::{anyhow, Result};
use std::fmt::Display;
use url::{ParseError, Url};

use crate::{authority::GatewayIdentity, data::AsQuery};

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
pub struct GatewayReference {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub identity: Option<GatewayIdentity>,
}

impl GatewayReference {
    pub fn try_from_uri(uri: &str) -> Result<GatewayReference> {
        let url = Url::parse(uri)?;
        println!("{:?}", url);
        Ok(GatewayReference {
            scheme: url.scheme().to_string(),
            host: url
                .host()
                .ok_or_else(|| anyhow!("Could not derive domain from {}", uri))?
                .to_string(),
            port: url.port().unwrap_or(80),
            identity: None,
        })
    }

    pub fn try_from_uri_and_did(uri: &str, did: &str) -> Result<GatewayReference> {
        let mut reference = GatewayReference::try_from_uri(uri)?;
        reference.identity = Some(GatewayIdentity {
            did: did.to_string(),
        });
        Ok(reference)
    }

    pub fn require_identity(&self) -> Result<&GatewayIdentity> {
        Ok(self
            .identity
            .as_ref()
            .ok_or_else(|| anyhow!("No DID configured for gateway identity"))?)
    }

    pub fn ensure_identity(&mut self, claimed_identity: &GatewayIdentity) -> Result<()> {
        match &self.identity {
            Some(identity) if identity != claimed_identity => {
                return Err(anyhow!(
                    "Gateway claimed identity {} but client expected {}",
                    claimed_identity,
                    identity
                ));
            }
            None => {
                self.identity = Some(claimed_identity.clone());
            }
            _ => (),
        };

        Ok(())
    }
}

impl TryFrom<&GatewayReference> for Url {
    type Error = url::ParseError;
    fn try_from(origin: &GatewayReference) -> Result<Self, Self::Error> {
        Url::parse(&format!(
            "{}://{}:{}",
            origin.scheme, origin.host, origin.port
        ))
    }
}

pub struct GatewayRequestUrl<'a, 'b, Params: AsQuery = ()>(
    pub &'a GatewayReference,
    pub Route,
    pub Option<&'b Params>,
);

impl<'a, 'b, Params: AsQuery> TryFrom<GatewayRequestUrl<'a, 'b, Params>> for Url {
    type Error = ParseError;

    fn try_from(value: GatewayRequestUrl<'a, 'b, Params>) -> Result<Self, Self::Error> {
        let GatewayRequestUrl(
            GatewayReference {
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
