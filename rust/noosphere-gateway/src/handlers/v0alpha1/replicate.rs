use std::pin::Pin;

use anyhow::Result;
use axum::{
    body::StreamBody,
    extract::{Path, Query},
    http::StatusCode,
    Extension,
};
use bytes::Bytes;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_core::api::v0alpha1::ReplicateParameters;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_core::stream::{memo_body_stream, memo_history_stream, to_car_stream};
use noosphere_core::{
    authority::{generate_capability, SphereAbility},
    data::{ContentType, MemoIpld},
};
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_storage::{BlockStore, BlockStoreRetry, Storage};
use tokio_stream::Stream;

use crate::{authority::GatewayAuthority, GatewayScope};

pub type ReplicationCarStreamBody =
    StreamBody<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>>;

/// Invoke to get a streamed CARv1 response that represents all the blocks
/// needed to manifest the content associated with the given CID path parameter.
/// The CID should refer to the memo that wraps the content. The content-type
/// header is used to determine how to gather the associated blocks to be
/// streamed by to the requesting client. Invoker must have authorization to
/// fetch from the gateway.
#[instrument(level = "debug", skip(authority, scope, sphere_context,))]
pub async fn replicate_route<C, S>(
    authority: GatewayAuthority<S>,
    // NOTE: Cannot go from string to CID via serde
    Path(memo_version): Path<String>,
    Query(ReplicateParameters { since }): Query<ReplicateParameters>,
    Extension(scope): Extension<GatewayScope>,
    Extension(ipfs_client): Extension<KuboClient>,
    Extension(sphere_context): Extension<C>,
) -> Result<ReplicationCarStreamBody, StatusCode>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    debug!("Invoking replicate route...");

    let memo_version = Cid::try_from(memo_version).map_err(|error| {
        warn!("{}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    authority.try_authorize(&generate_capability(
        &scope.counterpart,
        SphereAbility::Fetch,
    ))?;

    let db = sphere_context
        .sphere_context()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .db()
        .clone();
    let store = BlockStoreRetry::from(IpfsStore::new(db, Some(ipfs_client)));

    if let Some(since) = since {
        let since_memo = store
            .load::<DagCborCodec, MemoIpld>(&since)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let latest_memo = store
            .load::<DagCborCodec, MemoIpld>(&memo_version)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if since_memo.lamport_order() < latest_memo.lamport_order() {
            // We should attempt to replicate incrementally
            if is_allowed_to_replicate_incrementally(&since_memo, &latest_memo) {
                // TODO(#408): To mitigate abuse, we should probably cap the spread
                // between revisions to some finite value that is permissive of 99%
                // of usage. Maybe somewhere in the ballpark of 1~10k revisions. It
                // should be a large-but-finite number.
                debug!("Streaming revisions from {} to {}", since, memo_version);
                return Ok(StreamBody::new(Box::pin(to_car_stream(
                    vec![memo_version],
                    memo_history_stream(store, &memo_version.into(), Some(&since), false),
                ))));
            } else {
                error!("Suggested version {since} is not a valid ancestor of {memo_version}");
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            warn!(
                "Client's version {} is not older than {}; can't stream incrementally",
                since, memo_version
            );
        }
    }

    debug!("Streaming entire version for {}", memo_version);

    // Always fall back to a full replication
    Ok(StreamBody::new(Box::pin(to_car_stream(
        vec![memo_version],
        memo_body_stream(store, &memo_version.into(), false),
    ))))
}

/// Perform verification to ensure that there is a valid lineage to be sought
/// between two memos. This helps to ensure that malformed or malicious
/// replication requests do not have the ability to DoS the gateway or cause
/// unwanted blocks to be sent to the client.
fn is_allowed_to_replicate_incrementally(since_memo: &MemoIpld, latest_memo: &MemoIpld) -> bool {
    // Ensure that we are talking about a sphere by checking the "content-type"
    // headers in both memos
    if since_memo.content_type() != Some(ContentType::Sphere)
        || latest_memo.content_type() != Some(ContentType::Sphere)
    {
        return false;
    }

    // Verify that the causal order of the memos is what we expect
    if since_memo.lamport_order() >= latest_memo.lamport_order() {
        return false;
    }

    // Get the "proof" UCANs that are required for a sphere header
    let since_ucan = if let Ok(ucan) = since_memo.require_proof() {
        ucan
    } else {
        return false;
    };

    let latest_ucan = if let Ok(ucan) = latest_memo.require_proof() {
        ucan
    } else {
        return false;
    };

    // TODO(#407): In order to circumvent abuse, we actually need to verify 1)
    // the chains for both UCANs are valid and 2) no witnesses in the chain have
    // had their authority revoked

    // Verify that we have the same audience, presumed to be the identity of the sphere
    if since_ucan.audience() != latest_ucan.audience() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::is_allowed_to_replicate_incrementally;
    use anyhow::Result;
    use noosphere_core::authority::Access;
    use noosphere_core::context::{
        HasMutableSphereContext, HasSphereContext, SphereContext, SphereContextKey, SphereCursor,
    };
    use noosphere_core::helpers::simulated_sphere_context;
    use noosphere_core::{
        authority::{
            generate_capability, generate_ed25519_key, Author, Authorization, SphereAbility,
        },
        data::{DelegationIpld, RevocationIpld},
    };
    use tokio::sync::Mutex;
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial};

    #[tokio::test]
    async fn it_only_allows_incremental_replication_of_causally_ordered_revisions() -> Result<()> {
        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let db = sphere_context.sphere_context().await?.db().clone();

        let version_1 = sphere_context
            .save(Some(vec![("V".into(), "1".into())]))
            .await?;

        let version_2 = sphere_context
            .save(Some(vec![("V".into(), "2".into())]))
            .await?;

        let version_3 = sphere_context
            .save(Some(vec![("V".into(), "3".into())]))
            .await?;

        let memo_1 = version_1.load_from(&db).await?;
        let memo_2 = version_2.load_from(&db).await?;
        let memo_3 = version_3.load_from(&db).await?;

        assert!(is_allowed_to_replicate_incrementally(&memo_1, &memo_3));
        assert!(is_allowed_to_replicate_incrementally(&memo_2, &memo_3));
        assert!(!is_allowed_to_replicate_incrementally(&memo_2, &memo_2));
        assert!(!is_allowed_to_replicate_incrementally(&memo_3, &memo_2));
        assert!(!is_allowed_to_replicate_incrementally(&memo_3, &memo_1));

        Ok(())
    }

    #[tokio::test]
    async fn it_only_allows_incremental_replication_of_revisions_from_same_sphere() -> Result<()> {
        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let db = sphere_context.sphere_context().await?.db().clone();

        let version_1 = sphere_context
            .save(Some(vec![("V".into(), "1".into())]))
            .await?;

        let version_2 = sphere_context
            .save(Some(vec![("V".into(), "2".into())]))
            .await?;

        let memo_1 = version_1.load_from(&db).await?;
        let memo_2 = version_2.load_from(&db).await?;

        let (mut other_sphere_context, _) =
            simulated_sphere_context(Access::ReadWrite, Some(db.clone())).await?;

        let other_version_1 = other_sphere_context
            .save(Some(vec![("V".into(), "1".into())]))
            .await?;

        let other_version_2 = other_sphere_context
            .save(Some(vec![("V".into(), "2".into())]))
            .await?;

        let other_memo_1 = other_version_1.load_from(&db).await?;
        let other_memo_2 = other_version_2.load_from(&db).await?;

        assert!(is_allowed_to_replicate_incrementally(&memo_1, &memo_2));
        assert!(is_allowed_to_replicate_incrementally(
            &other_memo_1,
            &other_memo_2
        ));
        assert!(!is_allowed_to_replicate_incrementally(
            &other_memo_1,
            &memo_2
        ));
        assert!(!is_allowed_to_replicate_incrementally(
            &memo_1,
            &other_memo_2
        ));
        assert!(!is_allowed_to_replicate_incrementally(
            &other_memo_2,
            &memo_1
        ));
        assert!(!is_allowed_to_replicate_incrementally(
            &memo_2,
            &other_memo_1
        ));

        Ok(())
    }

    #[tokio::test]
    #[ignore = "TODO(#407)"]
    async fn it_detects_forked_lineages_of_a_sphere_by_revoked_authors() -> Result<()> {
        let to_be_revoked_key: SphereContextKey = Arc::new(Box::new(generate_ed25519_key()));
        let to_be_revoked_did = to_be_revoked_key.get_did().await?;

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let db = sphere_context.sphere_context().await?.db().clone();

        let _ = sphere_context
            .save(Some(vec![("V".into(), "1".into())]))
            .await?;

        let author = sphere_context.sphere_context().await?.author().clone();
        let author_key = author.key.clone();
        let author_ucan = author.require_authorization()?.as_ucan(&db).await?;

        let to_be_revoked_jwt = UcanBuilder::default()
            .issued_by(&author_key)
            .for_audience(&to_be_revoked_did)
            .claiming_capability(&generate_capability(
                &sphere_context.identity().await?,
                SphereAbility::Publish,
            ))
            .witnessed_by(&author_ucan, None)
            .with_lifetime(6000)
            .with_nonce()
            .build()?
            .sign()
            .await?
            .encode()?;

        let delegation = DelegationIpld::register("to_be_revoked", &to_be_revoked_jwt, &db).await?;

        sphere_context
            .sphere_context_mut()
            .await?
            .mutation_mut()
            .delegations_mut()
            .set(&delegation.jwt.into(), &delegation);

        let version_2 = sphere_context
            .save(Some(vec![("V".into(), "2".into())]))
            .await?;

        let revocation = RevocationIpld::revoke(&delegation.jwt, &author_key).await?;

        sphere_context
            .sphere_context_mut()
            .await?
            .mutation_mut()
            .revocations_mut()
            .set(&delegation.jwt.into(), &revocation);

        sphere_context
            .sphere_context_mut()
            .await?
            .mutation_mut()
            .delegations_mut()
            .remove(&delegation.jwt.into());

        let _ = sphere_context
            .save(Some(vec![("V".into(), "3".into())]))
            .await?;

        let version_4 = sphere_context
            .save(Some(vec![("V".into(), "4".into())]))
            .await?;

        let rogue_author = Author {
            key: to_be_revoked_key,
            authorization: Some(Authorization::Cid(delegation.jwt)),
        };

        let rogue_sphere_context = SphereContext::new(
            sphere_context.identity().await?,
            rogue_author,
            db.clone(),
            None,
        )
        .await?;

        let mut rogue_cursor =
            SphereCursor::mounted_at(Arc::new(Mutex::new(rogue_sphere_context)), &version_2);

        let rogue_version_3 = rogue_cursor
            .save(Some(vec![("V".into(), "3".into())]))
            .await?;

        let memo_2 = version_2.load_from(&db).await?;
        let memo_4 = version_4.load_from(&db).await?;

        let rogue_memo_3 = rogue_version_3.load_from(&db).await?;

        assert!(!is_allowed_to_replicate_incrementally(
            &memo_2,
            &rogue_memo_3
        ));
        assert!(!is_allowed_to_replicate_incrementally(
            &rogue_memo_3,
            &memo_4
        ));

        Ok(())
    }
}
