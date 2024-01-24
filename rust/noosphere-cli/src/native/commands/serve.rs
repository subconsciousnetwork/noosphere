//! Concrete implementations of subcommands related to running a Noosphere
//! gateway server

use crate::native::workspace::Workspace;
use anyhow::Result;
use noosphere_core::context::HasSphereContext;
use noosphere_gateway::{Gateway, SingleTenantGatewayManager};
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
    let sphere_context = workspace.sphere_context().await?;
    let gateway_identity = sphere_context
        .sphere_context()
        .await?
        .author()
        .did()
        .await?;
    let manager = SingleTenantGatewayManager::new(
        sphere_context,
        counterpart.clone(),
        ipfs_api,
        name_resolver_api,
        cors_origin,
    )
    .await?;

    let gateway = Gateway::new(manager)?;

    info!(
        r#"A geist is summoned to manage local sphere {}

    It has bound a gateway to {:?}
    It awaits updates from sphere {}..."#,
        gateway_identity,
        listener
            .local_addr()
            .expect("Unexpected missing listener address"),
        counterpart
    );

    gateway.start(listener).await?;
    Ok(())
}
