use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use ucan::{
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::did::DidParser,
    ucan::Ucan,
};

use crate::data::{ContentType, Header, MemoIpld, SphereIpld};

use noosphere_storage::{encoding::base64_decode, interface::BlockStore, ucan::UcanStore};

use crate::authority::SPHERE_SEMANTICS;

use super::{SphereAction, SphereReference};

pub async fn verify_sphere_cid<S: BlockStore>(
    cid: &Cid,
    store: &S,
    did_parser: &mut DidParser,
) -> Result<()> {
    let memo = store.load::<DagCborCodec, MemoIpld>(cid).await?;

    // Ensure that we have the correct content type
    memo.expect_header(
        &Header::ContentType.to_string(),
        &ContentType::Sphere.to_string(),
    )?;

    // Extract signature from the eponimous header
    let signature_header = memo
        .get_header(&Header::Signature.to_string())
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("No signature header found"))?;

    let signature = base64_decode(&signature_header)?;

    // Load up the sphere being verified
    let sphere = store.load::<DagCborCodec, SphereIpld>(&memo.body).await?;

    // If we have an authorizing proof...
    if let Some(proof_header) = memo.get_header(&Header::Proof.to_string()).first() {
        let ucan_store = UcanStore(store.clone());

        // Extract a UCAN from the proof header, or...
        let ucan = Ucan::try_from_token_string(proof_header)?;

        // Discover the intended audience of the UCAN
        let credential = did_parser.parse(ucan.audience())?;

        // Verify the audience signature of the body CID
        credential.verify(&memo.body.to_bytes(), &signature).await?;

        // Check the proof's provenance and that it enables the signer to sign
        let proof = ProofChain::from_ucan(ucan, did_parser, &ucan_store).await?;

        let desired_capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: sphere.identity.clone(),
                }),
            },
            can: SphereAction::Push,
        };

        for capability_info in proof.reduce_capabilities(&SPHERE_SEMANTICS) {
            let capability = capability_info.capability;
            if capability_info.originators.contains(&sphere.identity)
                && capability.enables(&desired_capability)
            {
                return Ok(());
            }
        }

        Err(anyhow!("Proof did not enable signer to sign this sphere"))
    } else {
        // Assume the identity is the signer
        let credential = did_parser.parse(&sphere.identity)?;

        // Verify the identity signature of the body CID
        credential.verify(&memo.body.to_bytes(), &signature).await?;

        Ok(())
    }
}
