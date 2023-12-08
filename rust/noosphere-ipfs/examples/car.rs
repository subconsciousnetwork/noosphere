//! Simple utility to verify the contents of a .car file using the same
//! CAR-reading facilities in use by Noosphere more generally

#[cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use iroh_car::CarReader;

use noosphere_core::stream::BlockLedger;
use noosphere_ipfs::debug::debug_block;

#[cfg(not(target_arch = "wasm32"))]
use tokio::fs::File;

#[cfg(target_arch = "wasm32")]
pub fn main() {}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(target_arch = "wasm32"), tokio::main)]
pub async fn main() -> Result<()> {
    let file = if let Some(arg) = env::args().nth(1) {
        println!("Opening {arg}...\n");
        File::open(arg).await?
    } else {
        println!("Please specify a path to a CARv1 file");
        std::process::exit(1);
    };

    let mut reader = CarReader::new(file).await?;

    let header = reader.header().clone();

    println!("=== Header (CARv{}) ===\n", header.version());

    for root in header.roots() {
        println!("{}", root);
    }

    println!();

    let mut index = 0usize;

    let mut block_ledger = BlockLedger::default();

    while let Some((cid, block)) = reader.next_block().await? {
        println!("=== Block {} ===\n", index);

        block_ledger.record(&cid, &block)?;

        let mut out = String::new();
        debug_block(&cid, &block, &mut out)?;
        println!("{out}");

        index += 1;
    }

    let missing_references = block_ledger
        .missing_references()
        .into_iter()
        .map(|cid| cid.to_string())
        .collect::<Vec<String>>();

    let orphaned = block_ledger
        .orphans()
        .into_iter()
        .filter_map(|cid| {
            if header.roots().contains(cid) {
                None
            } else {
                Some(cid.to_string())
            }
        })
        .collect::<Vec<String>>();

    if !missing_references.is_empty() {
        println!("=== References to missing blocks ===\n");
        println!("{}\n", missing_references.join("\n"));
    }

    if !orphaned.is_empty() {
        println!("=== Orphaned blocks ===\n");
        println!("{}\n", orphaned.join("\n"));
    }

    Ok(())
}
