mod cli;
use crate::cli::{keyname_to_keymaterial, read_pinned_file, CLICommand, QueryCommand, CLI};
use anyhow::Result;
use clap::Parser;
use futures::future::try_join_all;
use noosphere::authority::generate_ed25519_key;
use noosphere_cli::native::workspace::Workspace;
use noosphere_name_system::NameSystem;
use tokio;
use tokio::signal;

const QUERY_TIMEOUT: u32 = 5 * 60;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = CLI::parse();
    let workspace = Workspace::new(&std::env::current_dir()?)?;
    workspace.initialize_global_directories().await?;

    match cli.command {
        CLICommand::Daemon { keyname, pinned } => {
            println!("Using keyname {}", keyname);
            let key_material = keyname_to_keymaterial(&workspace, &keyname).await?;
            let pinned_list = read_pinned_file(&pinned)?;
            let mut ns = NameSystem::new(&key_material, QUERY_TIMEOUT)?;
            ns.connect().await?;

            if let Some(pinned) = pinned_list {
                let ns_ref = &ns;
                let pending_responses: Vec<_> = pinned
                    .iter()
                    .map(|record| {
                        println!("Pinning {} key {}...", record.0, record.1);
                        ns_ref.set_record(
                            record.1.clone().into_bytes(),
                            record.2.clone().into_bytes(),
                        )
                    })
                    .collect();

                let pinned_names = try_join_all(pending_responses).await?;
                for pinned_name in pinned_names {
                    println!("Pinned {:#?}", pinned_name);
                }
            }
            signal::ctrl_c().await?;
            ns.disconnect().await?;
            Ok(())
        }
        CLICommand::Query { command } => match command {
            QueryCommand::Get { name } => {
                // Just querying, use an ephemeral key
                let key_material = generate_ed25519_key();
                let mut ns = NameSystem::new(&key_material, QUERY_TIMEOUT)?;
                ns.connect().await?;
                let result = ns.get_record(name.clone().into_bytes()).await?;
                match result {
                    Some(value) => {
                        println!(
                            "Found record for {}: {}",
                            name,
                            String::from_utf8(value).unwrap()
                        )
                    }
                    None => println!(
                        "Query completed successfully, but no results found for {}",
                        name
                    ),
                }
                Ok(())
            }
        },
    }
}
