use anyhow::Result;
use std::fmt::Write;

use cid::multihash;
use cid::multihash::MultihashDigest;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use libipld_core::ipld::Ipld;
use libipld_core::raw::RawCodec;
use noosphere_storage::block_decode;

pub fn hash_for(cid: &Cid) -> &'static str {
    // let multihash = cid::multihash;

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

pub fn codec_for(cid: &Cid) -> &'static str {
    match cid.codec() {
        codec if codec == u64::from(DagCborCodec) => "DAG-CBOR",
        codec if codec == u64::from(RawCodec) => "Raw",
        _ => "Other",
    }
}

pub fn debug_block<W>(cid: &Cid, block: &[u8], out: &mut W) -> Result<()>
where
    W: Write,
{
    let verification_sign =
        if cid.codec() == u64::from(DagCborCodec) || cid.codec() == u64::from(RawCodec) {
            let hasher = multihash::Code::try_from(cid.hash().code()).ok();

            if let Some(hasher) = hasher {
                let multihash = hasher.digest(&block);
                let new_cid = Cid::new_v1(cid.codec(), multihash);

                if cid == &new_cid {
                    "✔️"
                } else {
                    "🚫"
                }
            } else {
                "🤷"
            }
        } else {
            "🤷"
        };

    writeln!(
        out,
        "{} {} ({:?}, {}, {})\n",
        verification_sign,
        cid,
        cid.version(),
        hash_for(cid),
        codec_for(cid)
    )?;
    writeln!(
        out,
        "{}\n",
        block
            .iter()
            .map(|byte| format!("{:02X?}", byte))
            .collect::<Vec<String>>()
            .join(" ")
    )?;

    if cid.codec() == u64::from(DagCborCodec) {
        if let Some(ipld) = block_decode::<DagCborCodec, Ipld>(&block).ok() {
            writeln!(out, "{:#?}\n", ipld)?;
        }
    };

    Ok(())
}
