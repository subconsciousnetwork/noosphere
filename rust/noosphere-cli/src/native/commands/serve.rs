//! Concrete implementations of subcommands related to running a Noosphere
//! gateway server

use crate::native::workspace::Workspace;
use anyhow::Result;
use noosphere_gateway::{start_gateway, GatewayScope};
use std::net::{IpAddr, TcpListener};
use url::Url;

/// Start a Noosphere gateway server
pub async fn serve(
    interface: IpAddr,
    port: u16,
    ipfs_api: Url,
    name_resolver_api: Url,
    cors_origin: Option<Url>,
    workspace: &mut Workspace,
) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let listener = TcpListener::bind((interface, port))?;

    let counterpart = workspace.counterpart_identity().await?;

    let identity = workspace.sphere_identity().await?;

    let gateway_scope = GatewayScope {
        identity,
        counterpart,
    };

    let sphere_context = workspace.sphere_context().await?;

    start_gateway(
        listener,
        gateway_scope,
        sphere_context,
        ipfs_api,
        name_resolver_api,
        cors_origin,
    )
    .await
}
