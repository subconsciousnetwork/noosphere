use crate::jobs::GatewayJob;
use anyhow::{anyhow, Result};
use noosphere_core::{
    context::{HasMutableSphereContext, HasSphereContext, SphereCursor, COUNTERPART},
    data::Did,
};
use noosphere_storage::{KeyValueStore, Storage};
use tokio_stream::StreamExt;

/// Compact a sphere's history.
pub async fn compact_sphere<C, S>(context: C) -> Result<Option<GatewayJob>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    let mut cursor = SphereCursor::latest(context);
    let author = cursor.sphere_context().await?.author().clone();
    let sphere_identity = cursor.identity().await?;

    debug!(
        "Attempting history compaction for local sphere {}",
        sphere_identity
    );

    let counterpart: Did = cursor
        .sphere_context()
        .await?
        .db()
        .require_key(COUNTERPART)
        .await?;

    // Look at the parent of the oldest gateway sphere version we have
    // checked so far; if that parent has a content changelog that
    // contains a change to the counterpart sphere root, that's the new
    // base, aka the intended parent version of the compact change we
    // are about to produce.
    let (compact_until, version_count) = {
        let mut version_count = 0usize;

        let sphere = cursor.to_sphere().await?;
        let stream = sphere.into_history_stream(None);

        tokio::pin!(stream);

        let mut compact_until = None;

        while let Some((cid, sphere)) = stream.try_next().await? {
            let counterpart_changed = sphere
                .get_content()
                .await?
                .get_changelog()
                .await?
                .changes
                .iter()
                .filter(|op| {
                    let key = match op {
                        noosphere_core::data::MapOperation::Add { key, .. } => key,
                        noosphere_core::data::MapOperation::Remove { key } => key,
                    };
                    key == &counterpart
                })
                .count()
                > 0;

            if counterpart_changed {
                break;
            }

            compact_until = Some(cid);
            version_count += 1;
        }

        (compact_until, version_count)
    };

    // Here we perform the actual compaction, so we take a mutable lock
    // on the sphere context until we are done
    if let Some(compact_until) = compact_until {
        debug!("Compacting {version_count} versions (through {compact_until})",);

        let cursor_version = cursor.version().await?;
        let mut context = cursor.sphere_context_mut().await?;
        let latest_version = context.version().await?;

        if cursor_version != latest_version {
            return Err(anyhow!(
                "Could not compact history; history advanced since job began"
            ));
        }

        let sphere = context.sphere().await?;
        let new_tip = sphere.compact(&compact_until, &author).await?;
        context
            .db_mut()
            .set_version(&sphere_identity, &new_tip)
            .await?;

        debug!("Finished compacting {version_count} versions; new tip is {new_tip}");
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use noosphere_core::{
        authority::Access,
        context::{
            HasMutableSphereContext, HasSphereContext, SphereContentWrite, SphereCursor,
            COUNTERPART,
        },
        data::ContentType,
        helpers::{make_valid_link_record, simulated_sphere_context},
        tracing::initialize_tracing,
        view::Timeline,
    };
    use noosphere_storage::KeyValueStore;

    #[tokio::test]
    async fn it_compacts_excess_name_record_changes_in_a_gateway_sphere() -> Result<()> {
        initialize_tracing(None);

        let (mut gateway_sphere_context, _) =
            simulated_sphere_context(Access::ReadWrite, None).await?;
        let mut gateway_db = gateway_sphere_context
            .sphere_context()
            .await?
            .db_mut()
            .clone();
        let (user_sphere_context, _) =
            simulated_sphere_context(Access::ReadWrite, Some(gateway_db.clone())).await?;
        let user_sphere_identity = user_sphere_context.identity().await?;
        let user_sphere_version = user_sphere_context.version().await?;

        gateway_db
            .set_key(COUNTERPART, &user_sphere_identity)
            .await?;
        gateway_sphere_context
            .link_raw(&format!("{user_sphere_identity}"), &user_sphere_version)
            .await?;
        let base_version = gateway_sphere_context.save(None).await?;

        debug!("Base version: {}", base_version);

        let tl = Timeline::new(&gateway_db);
        let ts = tl.slice(&base_version, None);
        let versions = ts.to_chronological().await?;

        debug!(
            "Before task: {:#?}",
            versions
                .iter()
                .map(|cid| cid.to_string())
                .collect::<Vec<String>>()
        );

        let mut latest_version = base_version;

        for _ in 0..10 {
            let (_, link_record, _) = make_valid_link_record(&mut gateway_db.clone()).await?;
            gateway_sphere_context
                .write(
                    &format!("link_record/{user_sphere_identity}"),
                    &ContentType::Text,
                    link_record.encode()?.as_bytes(),
                    None,
                )
                .await?;
            latest_version = gateway_sphere_context.save(None).await?;
        }

        let ts = tl.slice(&latest_version, None);
        let versions = ts.to_chronological().await?;

        debug!(
            "Before compaction: {:#?}",
            versions
                .iter()
                .map(|cid| cid.to_string())
                .collect::<Vec<String>>()
        );

        assert_eq!(13, versions.len());

        debug!("Test proceeding");

        let _ = compact_sphere(gateway_sphere_context.clone()).await?;

        let cursor = SphereCursor::latest(gateway_sphere_context);
        let new_latest_version = cursor.version().await?;

        debug!("New latest version: {}", new_latest_version);

        assert_ne!(new_latest_version, latest_version);

        let ts = tl.slice(&new_latest_version, None);
        let versions = ts.to_chronological().await?;

        debug!(
            "After compaction: {:#?}",
            versions
                .iter()
                .map(|cid| cid.to_string())
                .collect::<Vec<String>>()
        );

        assert_eq!(4, versions.len());

        assert_eq!(
            cursor.to_sphere().await?.get_parent().await?.unwrap().cid(),
            &base_version
        );

        Ok(())
    }
}
