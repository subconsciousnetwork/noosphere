use anyhow::Result;
use cid::Cid;
use noosphere_ucan::{store::UcanJwtStore, Ucan};

/// Walk a [Ucan] and collect all of the supporting proofs that
/// verify the link publisher's authority to publish the link
#[instrument(level = "trace", skip(store))]
pub async fn collect_ucan_proofs<S>(ucan: &Ucan, store: &S) -> Result<Vec<Ucan>>
where
    S: UcanJwtStore,
{
    let mut proofs = vec![];
    let mut remaining = vec![ucan.clone()];

    while let Some(ucan) = remaining.pop() {
        if let Some(ucan_proofs) = ucan.proofs() {
            for proof_cid_string in ucan_proofs {
                let cid = Cid::try_from(proof_cid_string.as_str())?;
                trace!("Collecting proof with CID {}", cid);
                let jwt = store.require_token(&cid).await?;
                let ucan = Ucan::try_from(jwt.as_str())?;

                remaining.push(ucan);
            }
        };

        proofs.push(ucan);
    }

    Ok(proofs)
}
