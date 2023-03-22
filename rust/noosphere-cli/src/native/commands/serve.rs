use anyhow::Result;

use std::net::{IpAddr, TcpListener};

use url::Url;

use crate::native::workspace::Workspace;

use noosphere_gateway::{start_gateway, GatewayScope};

pub async fn serve(
    interface: IpAddr,
    port: u16,
    ipfs_api: Url,
    name_resolver_api: Url,
    cors_origin: Option<Url>,
    workspace: &Workspace,
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
