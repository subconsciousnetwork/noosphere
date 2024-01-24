use crate::jobs::GatewayJob;
use anyhow::anyhow;
use anyhow::Result;
use noosphere_core::{
    context::{
        HasMutableSphereContext, SphereContentRead, SphereContentWrite, SphereCursor,
        SpherePetnameRead, SpherePetnameWrite, COUNTERPART,
    },
    data::{ContentType, Did, IdentityIpld, Link, LinkRecord, MapOperation, MemoIpld},
};
use noosphere_ipfs::{IpfsClient, IpfsStore};
use noosphere_ns::NameResolver;
use noosphere_storage::{BlockStoreRetry, KeyValueStore, Storage, UcanStore};
use std::collections::{BTreeMap, BTreeSet};
use tokio::io::AsyncReadExt;
use tokio_stream::{Stream, StreamExt};

/// Publish a record to the name system.
pub async fn name_system_publish<C, S, N>(
    context: C,
    ns_client: N,
    record: LinkRecord,
) -> Result<Option<GatewayJob>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + 'static,
{
    // NOTE: Very important not to update this record on every
    // re-publish, otherwise we will generate a new sphere revision
    // on an on-going basis indefinitely
    if let Err(error) = set_counterpart_record(context, &record).await {
        warn!("Could not set counterpart record on sphere: {error}");
    }
    if record.has_publishable_timeframe() {
        ns_client.publish(record).await?;
    } else {
        return Err(anyhow!("Record is expired and cannot be published."));
    }
    Ok(None)
}

/// Republish a record to the name system.
pub async fn name_system_republish<C, S, N>(context: C, ns_client: N) -> Result<Option<GatewayJob>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + 'static,
{
    match get_counterpart_record(&context).await {
        Ok(Some(record)) => {
            ns_client.publish(record).await?;
            Ok(None)
        }
        _ => {
            warn!("Could not find most recent record for counterpart sphere to publish.");
            Ok(None)
        }
    }
}

/// Resolve the address book for the given context.
pub async fn name_system_resolve_all<C, S, N, I>(
    context: C,
    ipfs_client: I,
    ns_client: N,
) -> Result<Option<GatewayJob>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone + 'static,
    I: IpfsClient + Send + Sync + 'static,
{
    let name_stream = {
        let sphere = context.to_sphere().await?;
        let names = sphere.get_address_book().await?.get_identities().await?;

        names.into_stream().await?
    };

    resolve_all(ns_client, context, name_stream, ipfs_client).await?;
    Ok(None)
}

/// Resolve the address book since the revision given for the given context.
pub async fn name_system_resolve_since<C, S, N, I>(
    context: C,
    ipfs_client: I,
    ns_client: N,
    since: Option<Link<MemoIpld>>,
) -> Result<Option<GatewayJob>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone + 'static,
    I: IpfsClient + Send + Sync + 'static,
{
    let history_stream = {
        let sphere = context.to_sphere().await?;
        sphere.into_history_stream(since.as_ref())
    };

    tokio::pin!(history_stream);

    let mut names_to_resolve = BTreeMap::<String, IdentityIpld>::new();
    let mut names_to_ignore = BTreeSet::new();

    while let Some((_, sphere)) = history_stream.try_next().await? {
        let names = sphere.get_address_book().await?.get_identities().await?;
        let changelog = names.load_changelog().await?;

        for operation in changelog.changes.iter() {
            match operation {
                MapOperation::Add { key, value } => {
                    // Walking backwards through history, we will
                    // ignore any name changes where the name has
                    // either been updated or removed in the future
                    if !names_to_ignore.contains(key) && !names_to_resolve.contains_key(key) {
                        names_to_resolve.insert(key.clone(), value.clone());
                    }
                }
                MapOperation::Remove { key } => {
                    names_to_ignore.insert(key.clone());
                }
            };
        }
    }

    resolve_all(
        ns_client,
        context,
        tokio_stream::iter(names_to_resolve.into_iter().map(Ok)),
        ipfs_client,
    )
    .await?;

    Ok(None)
}

/// Consumes a stream of name / address tuples, resolving them one at a time
/// and updating the provided [SphereContext] with the latest resolved values
async fn resolve_all<C, S, N, I, St>(
    ns_client: N,
    mut context: C,
    stream: St,
    ipfs_client: I,
) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone + 'static,
    I: IpfsClient + Send + Sync + 'static,
    St: Stream<Item = Result<(String, IdentityIpld)>>,
{
    tokio::pin!(stream);

    let db = context.sphere_context().await?.db().clone();

    let ipfs_store = {
        let inner = db.clone();
        let inner = IpfsStore::new(inner, Some(ipfs_client));
        let inner = BlockStoreRetry::from(inner);
        UcanStore(inner)
    };

    while let Some((name, identity)) = stream.try_next().await? {
        let last_known_record = identity.link_record(&db).await;

        let next_record =
            match fetch_record(ns_client.clone(), name.clone(), identity.did.clone()).await? {
                Some(record) => {
                    // TODO(#257)
                    if false {
                        match record.validate(&ipfs_store).await {
                            Ok(_) => {}
                            Err(error) => {
                                error!("Failed record validation: {}", error);
                                continue;
                            }
                        }
                    }

                    match &last_known_record {
                        Some(last_known_record) => match last_known_record.superceded_by(&record) {
                            true => Some(record),
                            false => None,
                        },
                        None => Some(record),
                    }
                }
                None => {
                    // TODO(#259): Expire recorded value if we don't get an updated
                    // record after some designated TTL
                    continue;
                }
            };

        match &next_record {
            // TODO(#260): What if the resolved value is None?
            Some(record) if last_known_record != next_record => {
                debug!(
                    "Gateway adopting petname link record for '{}' ({}): {}",
                    name, identity.did, record
                );

                if let Some(current_record) = context.get_petname_record(&name).await? {
                    if current_record.get_link() == record.get_link() {
                        continue;
                    }
                }

                if let Err(e) = context.set_petname_record(&name, record).await {
                    warn!("Could not set petname link record: {}", e);
                    continue;
                }
            }
            _ => continue,
        }
    }

    if context.has_unsaved_changes().await? {
        context.save(None).await?;
    }

    Ok(())
}

/// Attempts to fetch a single link record from the name system.
async fn fetch_record<N>(ns_client: N, name: String, identity: Did) -> Result<Option<LinkRecord>>
where
    N: NameResolver + Clone + 'static,
{
    debug!("Resolving record '{}' ({})...", name, identity);
    Ok(match ns_client.resolve(&identity).await {
        Ok(Some(record)) => {
            debug!(
                "Resolved record for '{}' ({}): {}",
                name,
                identity,
                record.to_string()
            );
            Some(record)
        }
        Ok(None) => {
            warn!("No record found for {} ({})", name, identity);
            None
        }
        Err(error) => {
            warn!("Failed to resolve '{}' ({}): {:?}", name, identity, error);
            None
        }
    })
}

async fn set_counterpart_record<C, S>(context: C, record: &LinkRecord) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Setting counterpart record...");
    let counterpart_identity = {
        let sphere_context = context.sphere_context().await?;
        let db = sphere_context.db();
        db.require_key::<_, Did>(COUNTERPART).await?
    };
    let counterpart_link_record_key = format!("link_record/{counterpart_identity}");
    let mut cursor = SphereCursor::latest(context.clone());
    cursor
        .write(
            &counterpart_link_record_key,
            &ContentType::Text,
            record.encode()?.as_bytes(),
            None,
        )
        .await?;

    cursor.save(None).await?;
    Ok(())
}

async fn get_counterpart_record<C, S>(context: &C) -> Result<Option<LinkRecord>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Getting counterpart record...");
    let counterpart_identity = {
        let sphere_context = context.sphere_context().await?;
        let db = sphere_context.db();
        db.require_key::<_, Did>(COUNTERPART).await?
    };
    let counterpart_link_record_key = format!("link_record/{counterpart_identity}");

    let mut buffer = String::new();
    if let Some(mut file) = context.read(&counterpart_link_record_key).await? {
        file.contents.read_to_string(&mut buffer).await?;
        Ok(Some(LinkRecord::try_from(buffer)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere_core::{
        authority::{generate_capability, Access, SphereAbility},
        context::HasSphereContext,
        data::LINK_RECORD_FACT_NAME,
        helpers::simulated_sphere_context,
    };
    use noosphere_ns::helpers::KeyValueNameResolver;
    use noosphere_ucan::builder::UcanBuilder;

    #[tokio::test]
    async fn it_publishes_to_the_name_system() -> Result<()> {
        let (user_sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let (gateway_sphere_context, _) = simulated_sphere_context(
            Access::ReadWrite,
            Some(user_sphere_context.lock().await.db().clone()),
        )
        .await?;
        let user_sphere_identity = user_sphere_context.identity().await?;
        gateway_sphere_context
            .lock()
            .await
            .db_mut()
            .set_key(COUNTERPART, &user_sphere_identity)
            .await?;

        let record: LinkRecord = {
            let context = user_sphere_context.lock().await;
            let identity: &str = context.identity().into();
            UcanBuilder::default()
                .issued_by(&context.author().key)
                .for_audience(identity)
                .claiming_capability(&generate_capability(identity, SphereAbility::Publish))
                .with_lifetime(1000)
                .with_fact(
                    LINK_RECORD_FACT_NAME,
                    "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i".to_owned(),
                )
                .build()
                .unwrap()
                .sign()
                .await
                .unwrap()
                .into()
        };

        let expired: LinkRecord = {
            let context = user_sphere_context.lock().await;
            let identity: &str = context.identity().into();
            UcanBuilder::default()
                .issued_by(&context.author().key)
                .for_audience(identity)
                .claiming_capability(&generate_capability(identity, SphereAbility::Publish))
                .with_expiration(noosphere_ucan::time::now() - 1000)
                .with_fact(
                    LINK_RECORD_FACT_NAME,
                    "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i".to_owned(),
                )
                .build()
                .unwrap()
                .sign()
                .await
                .unwrap()
                .into()
        };

        let ns_client = KeyValueNameResolver::default();

        assert!(
            name_system_publish(gateway_sphere_context.clone(), ns_client.clone(), record)
                .await
                .is_ok(),
            "Valid, unexpired records should be publishable by a gateway"
        );

        assert!(
            name_system_publish(
                gateway_sphere_context.clone(),
                ns_client.clone(),
                expired.clone()
            )
            .await
            .is_err(),
            "Expired records should not be publishable by a gateway"
        );

        // Manually set expired record to test republishing
        set_counterpart_record(gateway_sphere_context.clone(), &expired).await?;

        let expected_sphere_version = gateway_sphere_context.version().await?;

        assert!(
            name_system_republish(gateway_sphere_context.clone(), ns_client.clone())
                .await
                .is_ok(),
            "Republished records however can be published if expired."
        );

        let final_sphere_version = gateway_sphere_context.version().await?;

        name_system_republish(gateway_sphere_context.clone(), ns_client.clone()).await?;
        assert_eq!(
            expected_sphere_version, final_sphere_version,
            "Republishing a link record should not create new sphere history"
        );

        Ok(())
    }
}
