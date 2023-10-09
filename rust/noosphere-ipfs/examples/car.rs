//! Simple utility to verify the contents of a .car file using the same
//! CAR-reading facilities in use by Noosphere more generally

#[cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use cid::Cid;
use iroh_car::CarReader;
use libipld_cbor::DagCborCodec;
use libipld_core::raw::RawCodec;
use multihash::MultihashDigest;

use noosphere_core::stream::BlockLedger;

#[cfg(not(target_arch = "wasm32"))]
use tokio::fs::File;

pub fn hash_for(cid: Cid) -> &'static str {
    match multihash::Code::try_from(cid.hash().code()) {
        Ok(multihash::Code::Blake3_256) => "BLAKE3",
        Ok(multihash::Code::Sha2_256) => "SHA-256",
        Ok(_) => "Other",
        Err(error) => {
            println!("ERROR: {}", error);
            "Error reading codec"
        }
    }
}

pub fn codec_for(cid: Cid) -> &'static str {
    match cid.codec() {
        codec if codec == u64::from(DagCborCodec) => "DAG-CBOR",
        codec if codec == u64::from(RawCodec) => "Raw",
        _ => "Other",
    }
}

#[cfg(target_arch = "wasm32")]
pub fn main() {}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(target_arch = "wasm32"), tokio::main)]
pub async fn main() -> Result<()> {
    use libipld_core::ipld::Ipld;
    use noosphere_storage::block_decode;

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

        let verification_sign =
            if cid.codec() == u64::from(DagCborCodec) || cid.codec() == u64::from(RawCodec) {
                let hasher = cid::multihash::Code::try_from(cid.hash().code())?;
                let multihash = hasher.digest(&block);
                let new_cid = Cid::new_v1(cid.codec(), multihash);

                if cid == new_cid {
                    "✔️"
                } else {
                    "🚫"
                }
            } else {
                "🤷"
            };

        println!(
            "{} {} ({:?}, {}, {})\n",
            verification_sign,
            cid,
            cid.version(),
            hash_for(cid),
            codec_for(cid)
        );
        println!(
            "{}\n",
            block
                .iter()
                .map(|byte| format!("{:02X?}", byte))
                .collect::<Vec<String>>()
                .join(" ")
        );

        if cid.codec() == u64::from(DagCborCodec) {
            let ipld = block_decode::<DagCborCodec, Ipld>(&block)?;
            println!("{:#?}\n", ipld);
        }

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
