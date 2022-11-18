use anyhow::{anyhow, Result};

use noosphere::key::{InsecureKeyStorage, KeyStorage};
use noosphere_ns::Multiaddr;
use std::future::Future;
use std::path::PathBuf;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// Runs `future` until a Ctrl+C signal is received either during
/// `future`'s execution, or after, or if `future` returns an `Err`.
pub async fn run_until_abort(future: impl Future<Output = Result<()>>) -> Result<()> {
    // Allow aborting (ctrl+c) during the initial run,
    // and after (when we want to wait exclusively for ctrl+c signal)
    let mut aborted = false;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => { aborted = true; },
        result = future => { result?; }
    };
    if !aborted {
        tokio::signal::ctrl_c().await?;
    }
    Ok(())
}

pub async fn get_key_material(
    key_storage: &InsecureKeyStorage,
    key_name: &str,
) -> Result<Ed25519KeyMaterial> {
    if let Some(km) = key_storage.read_key(key_name).await?.take() {
        Ok(km)
    } else {
        Err(anyhow!(
            "No key \"{}\" found in `~/.noosphere/keys/`.",
            key_name
        ))
    }
}

pub fn create_listening_address(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/tcp/{}", port)
        .parse()
        .expect("parseable")
}

pub fn get_keys_dir() -> Result<PathBuf> {
    Ok(home::home_dir()
        .ok_or_else(|| anyhow!("Could not discover home directory."))?
        .join(".noosphere"))
}

pub fn filter_bootstrap_peers(self_address: &Multiaddr, peers: &[Multiaddr]) -> Vec<Multiaddr> {
    peers
        .iter()
        .filter_map(|addr| {
            if addr != self_address {
                Some(addr.to_owned())
            } else {
                None
            }
        })
        .collect()
}
