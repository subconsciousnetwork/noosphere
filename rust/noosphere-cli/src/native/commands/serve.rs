//! Concrete implementations of subcommands related to running a Noosphere
//! gateway server

use crate::native::workspace::Workspace;
use anyhow::Result;
use noosphere_gateway::{start_gateway, DocTicket, GatewayScope};
use std::net::{IpAddr, TcpListener};
use url::Url;

/// Start a Noosphere gateway server
pub async fn serve(
    interface: IpAddr,
    port: u16,
    name_resolver_api: Url,
    ipfs_api: Option<Url>,
    iroh_ticket: Option<DocTicket>,
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
    let paths = workspace.require_sphere_paths()?;
    let sphere_path = paths.sphere();

    start_gateway(
        listener,
        gateway_scope,
        sphere_context,
        ipfs_api,
        iroh_ticket,
        sphere_path,
        name_resolver_api,
        cors_origin,
    )
    .await
}
