#![allow(dead_code)]

mod cli;
mod random;

pub use crate::helpers::random::*;
pub use cli::*;

use anyhow::Result;
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_storage::{BlockStoreRetry, MemoryStore, NativeStorage, UcanStore};
use std::{net::TcpListener, sync::Arc, time::Duration};
use tempfile::TempDir;

use noosphere_cli::{
    cli::ConfigSetCommand,
    commands::{key::key_create, sphere::config_set, sphere::sphere_create},
    workspace::Workspace,
};
use noosphere_core::data::Did;
use noosphere_gateway::{start_gateway, GatewayScope};
use noosphere_ns::{helpers::NameSystemNetwork, server::start_name_system_api_server};
use noosphere_sphere::{HasSphereContext, SphereContext};
use tokio::{sync::Mutex, task::JoinHandle};
use url::Url;

/// Produce a temporary [Workspace] suitable for use in tests. The [Workspace]
/// will be configured to use temporary directories that are deleted as soon as
/// the corresponding [TempDir]'s (also returned) are dropped. Every invocation
/// of this helper will produce a unique temporary workspace with its own
/// directories.
///
/// In the returned tuple `(TempDir, TempDir)`, the first is the local sphere
/// root directory, and the second represents the global Noosphere configuration
/// directory.
pub fn temporary_workspace() -> Result<(Workspace, (TempDir, TempDir))> {
    let root = TempDir::new()?;
    let global_root = TempDir::new()?;

    Ok((
        Workspace::new(root.path(), Some(global_root.path()))?,
        (root, global_root),
    ))
}

/// Wait for the specified number of seconds; uses [tokio::time::sleep], so this
/// will yield to the async runtime rather than block until the sleep time is
/// elapsed.
pub async fn wait(seconds: u64) {
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}

async fn start_gateway_for_workspace(
    workspace: &Workspace,
    client_sphere_identity: &Did,
    ipfs_url: &Url,
    ns_url: &Url,
) -> Result<(Url, JoinHandle<()>)> {
    let gateway_listener = TcpListener::bind("127.0.0.1:0")?;
    let gateway_address = gateway_listener.local_addr()?;
    let gateway_url = Url::parse(&format!(
        "http://{}:{}",
        gateway_address.ip(),
        gateway_address.port()
    ))?;

    let gateway_sphere_context = workspace.sphere_context().await?;

    let client_sphere_identity = client_sphere_identity.clone();
    let ns_url = ns_url.clone();
    let ipfs_url = ipfs_url.clone();

    let join_handle = tokio::spawn(async move {
        start_gateway(
            gateway_listener,
            GatewayScope {
                identity: gateway_sphere_context.identity().await.unwrap(),
                counterpart: client_sphere_identity,
            },
            gateway_sphere_context,
            ipfs_url,
            ns_url,
            None,
        )
        .await
        .unwrap()
    });

    Ok((gateway_url, join_handle))
}

pub async fn start_name_system_server(ipfs_url: &Url) -> Result<(Url, JoinHandle<()>)> {
    // TODO(#267)
    let use_validation = false;
    let store = if use_validation {
        let inner = MemoryStore::default();
        let inner = IpfsStore::new(inner, Some(KuboClient::new(ipfs_url).unwrap()));
        let inner = BlockStoreRetry::from(inner);
        let inner = UcanStore(inner);
        Some(inner)
    } else {
        None
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let url = Url::parse(format!("http://{}:{}", address.ip(), address.port()).as_str()).unwrap();

    Ok((
        url,
        tokio::spawn(async move {
            let mut network = NameSystemNetwork::generate(2, store).await.unwrap();
            let node = network.nodes_mut().pop().unwrap();
            start_name_system_api_server(Arc::new(node), listener)
                .await
                .unwrap();
        }),
    ))
}

/// Represents a single sphere and a corresponding workspace.
pub struct SphereData {
    pub identity: Did,
    pub workspace: Workspace,
    _temp_dirs: (tempfile::TempDir, tempfile::TempDir),
}

/// A test helper struct that represents both client and gateway spheres,
/// and provides high-level utility methods for synchronizing between
/// the two in order to support DSL-like integration tests.
pub struct SpherePair {
    pub name: String,
    pub client: SphereData,
    pub gateway: SphereData,
    pub ns_url: Url,
    pub ipfs_url: Url,
    gateway_url: Option<Url>,
    gateway_task: Option<JoinHandle<()>>,
}

impl SpherePair {
    /// Creates a new client and gateway sphere with workspace, and associates
    /// them together.
    pub async fn new(name: &str, ipfs_url: &Url, ns_url: &Url) -> Result<Self> {
        let (mut client_workspace, client_temp_dirs) = temporary_workspace()?;
        let (mut gateway_workspace, gateway_temp_dirs) = temporary_workspace()?;
        let client_key_name = format!("{}-CLIENT_KEY", name);
        let gateway_key_name = format!("{}-GATEWAY_KEY", name);
        key_create(&client_key_name, &client_workspace).await?;
        key_create(&gateway_key_name, &gateway_workspace).await?;
        sphere_create(&client_key_name, &mut client_workspace).await?;
        sphere_create(&gateway_key_name, &mut gateway_workspace).await?;
        let client_identity = client_workspace.sphere_identity().await.unwrap();
        let gateway_identity = gateway_workspace.sphere_identity().await.unwrap();

        config_set(
            ConfigSetCommand::Counterpart {
                did: client_identity.clone().into(),
            },
            &gateway_workspace,
        )
        .await?;
        let client = SphereData {
            identity: client_identity,
            workspace: client_workspace,
            _temp_dirs: client_temp_dirs,
        };
        let gateway = SphereData {
            identity: gateway_identity,
            workspace: gateway_workspace,
            _temp_dirs: gateway_temp_dirs,
        };
        Ok(SpherePair {
            name: name.into(),
            client,
            gateway,
            gateway_url: None,
            gateway_task: None,
            ipfs_url: ipfs_url.to_owned(),
            ns_url: ns_url.to_owned(),
        })
    }

    /// Starts the gateway service.
    pub async fn start_gateway(&mut self) -> Result<()> {
        if self.gateway_task.is_some() {
            return Err(anyhow::anyhow!("Gateway already started."));
        }
        let (gateway_url, gateway_task) = start_gateway_for_workspace(
            &self.gateway.workspace,
            &self.client.identity,
            &self.ipfs_url,
            &self.ns_url,
        )
        .await?;
        let client_sphere_context = self.client.workspace.sphere_context().await?;
        client_sphere_context
            .lock()
            .await
            .configure_gateway_url(Some(&gateway_url))
            .await?;
        self.gateway_url = Some(gateway_url);
        self.gateway_task = Some(gateway_task);
        Ok(())
    }

    /// Stops the gateway service.
    pub async fn stop_gateway(&mut self) -> Result<()> {
        if let Some(gateway_task) = self.gateway_task.take() {
            gateway_task.abort();
        } else {
            return Ok(());
        }
        self.gateway_url = None;

        let client_sphere_context = self.client.workspace.sphere_context().await?;
        client_sphere_context
            .lock()
            .await
            .configure_gateway_url(None)
            .await?;
        Ok(())
    }

    /// Returns a [SphereContext] for the client sphere.
    pub async fn sphere_context(&self) -> Result<Arc<Mutex<SphereContext<NativeStorage>>>> {
        self.client.workspace.sphere_context().await
    }

    pub async fn spawn<T, F, Fut>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Arc<Mutex<SphereContext<NativeStorage>>>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    {
        let context = self.sphere_context().await?;
        tokio::spawn(f(context)).await?
    }
}

impl Drop for SpherePair {
    fn drop(&mut self) {
        if let Some(gateway_task) = self.gateway_task.take() {
            gateway_task.abort();
        }
    }
}
