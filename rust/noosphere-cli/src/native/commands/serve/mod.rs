pub mod authority;
pub mod extractor;
pub mod gateway;
pub mod route;
pub mod tracing;

use anyhow::{anyhow, Result};

use std::net::{IpAddr, TcpListener};

use url::Url;

use crate::native::workspace::Workspace;

use self::gateway::GatewayScope;

use super::config::Config;

pub async fn serve(
    interface: IpAddr,
    port: u16,
    cors_origin: Option<Url>,
    workspace: &Workspace,
) -> Result<()> {
    workspace.expect_local_directories()?;

    let gateway_key = workspace.get_local_key().await?;
    let gateway_authorization = workspace.get_local_authorization().await?;
    let listener = TcpListener::bind(&(interface, port))?;
    let gateway_db = workspace.get_local_db().await?;

    let config = Config::from(workspace);

    let counterpart = match &config.read().await?.counterpart {
      Some(counterpart) => counterpart,
      None => return Err(anyhow!("No counterpart has been configured; you should set it to the DID of the sphere you are personally saving content to: orb config set counterpart <SOME_DID>"))
    };

    let identity = workspace.get_local_identity().await?;

    let gateway_scope = GatewayScope {
        identity,
        counterpart: counterpart.clone(),
    };

    gateway::start_gateway(
        listener,
        gateway_key,
        gateway_scope,
        gateway_authorization,
        gateway_db,
        cors_origin,
    )
    .await
}
