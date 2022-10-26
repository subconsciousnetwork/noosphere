use crate::cli::utils;
use anyhow::Result;
use noosphere_cli::native::workspace::Workspace;
use noosphere_p2p::{AddressBook, NameSystemBuilder};
use std::net::SocketAddr;
use tokio;
use tokio::signal;

pub async fn run_daemon(
    key_name: String,
    listening_address: SocketAddr,
    bootstrap_peers: Vec<String>,
    address_book_path: Option<std::path::PathBuf>,
    workspace: &Workspace,
) -> Result<()> {
    let key_material = utils::keyname_to_keymaterial(&workspace, &key_name).await?;
    let mut builder = NameSystemBuilder::default()
        .key_material(&key_material)
        .bootstrap_peers(&bootstrap_peers)
        .listening_address(listening_address);

    if let Some(addr_path) = address_book_path {
        builder = builder.address_book(AddressBook::from_path(&addr_path).await?);
    }

    let mut ns = builder.build()?;
    ns.connect().await?;
    println!(
        "Listening on {}",
        ns.p2p_address().expect("Active name system has an address")
    );
    info!("Awaiting for ctrl+c...");
    signal::ctrl_c().await?;
    ns.disconnect()?;
    Ok(())
}

pub async fn run_query(
    key_name: String,
    query_key: String,
    bootstrap_peers: Vec<String>,
    workspace: &Workspace,
) -> Result<Option<Vec<u8>>> {
    let key_material = utils::keyname_to_keymaterial(&workspace, &key_name).await?;
    let builder = NameSystemBuilder::default()
        .key_material(&key_material)
        .bootstrap_peers(&bootstrap_peers);

    let mut ns = builder.build()?;
    ns.connect().await?;

    if let Some(result) = ns.get_record(query_key.into_bytes()).await? {
        info!("Found result: {}", String::from_utf8(result.clone())?);
        Ok(Some(result))
    } else {
        info!("Query finished without errors, but no result found.");
        Ok(None)
    }
}
