#![cfg(all(feature = "test-kubo", not(target_arch = "wasm32")))]

//! Integration tests that expect "full stack" Noosphere to be available, including
//! name system and block syndication backend (e.g., IPFS Kubo). The tests in this
//! module represent sophisticated, complicated, nuanced or high-latency scenarios.

mod latency {
    // TODO(#629): Remove this when we migrate off of `release-please`
    extern crate noosphere_cli_dev as noosphere_cli;

    use anyhow::Result;
    use noosphere_cli::helpers::{start_name_system_server, SpherePair};
    use noosphere_common::helpers::TestEntropy;
    use noosphere_core::{
        context::{
            HasMutableSphereContext, SphereContentRead, SphereContentWrite, SpherePetnameWrite,
            SphereSync,
        },
        data::ContentType,
        tracing::initialize_tracing,
    };
    use rand::Rng;
    use tokio::io::AsyncReadExt;
    use url::Url;

    #[tokio::test(flavor = "multi_thread")]
    async fn clients_can_sync_when_there_is_a_lot_of_content() -> Result<()> {
        initialize_tracing(None);

        let entropy = TestEntropy::default();
        let rng = entropy.to_rng();

        let ipfs_url = Url::parse("http://127.0.0.1:5001").unwrap();
        let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await.unwrap();

        let mut pair_1 = SpherePair::new("ONE", &ipfs_url, &ns_url).await?;
        let mut pair_2 = SpherePair::new("TWO", &ipfs_url, &ns_url).await?;

        pair_1.start_gateway().await?;
        pair_2.start_gateway().await?;

        let peer_2_identity = pair_2.client.workspace.sphere_identity().await?;
        let pair_2_rng = rng.clone();

        pair_2
            .spawn(|mut ctx| async move {
                let mut rng = pair_2_rng.lock().await;

                // Long history, small-ish files
                for _ in 0..1000 {
                    let random_index = rng.gen_range(0..100);
                    let mut random_bytes = Vec::from(rng.gen::<[u8; 32]>());
                    let slug = format!("slug{}", random_index);

                    let next_bytes = if let Some(mut file) = ctx.read(&slug).await? {
                        let mut file_bytes = Vec::new();
                        file.contents.read_to_end(&mut file_bytes).await?;
                        file_bytes.append(&mut random_bytes);
                        file_bytes
                    } else {
                        random_bytes
                    };

                    ctx.write(&slug, &ContentType::Bytes, next_bytes.as_ref(), None)
                        .await?;
                    ctx.save(None).await?;
                }

                Ok(ctx.sync().await?)
            })
            .await?;

        let pair_1_rng = rng.clone();

        pair_1
            .spawn(|mut ctx| async move {
                let mut rng = pair_1_rng.lock().await;

                // Modest history, large-ish files
                for _ in 0..100 {
                    let mut random_bytes = (0..1000).fold(Vec::new(), |mut bytes, _| {
                        bytes.append(&mut Vec::from(rng.gen::<[u8; 32]>()));
                        bytes
                    });
                    let random_index = rng.gen_range(0..10);
                    let slug = format!("slug{}", random_index);

                    let next_bytes = if let Some(mut file) = ctx.read(&slug).await? {
                        let mut file_bytes = Vec::new();
                        file.contents.read_to_end(&mut file_bytes).await?;
                        file_bytes.append(&mut random_bytes);
                        file_bytes
                    } else {
                        random_bytes
                    };

                    ctx.write(&slug, &ContentType::Bytes, next_bytes.as_ref(), None)
                        .await?;

                    ctx.save(None).await?;
                }

                ctx.sync().await?;

                ctx.set_petname("peer2", Some(peer_2_identity)).await?;

                ctx.save(None).await?;

                ctx.sync().await?;

                // TODO(#606): Implement this part of the test when we "fix" latency asymmetry between
                // name system and syndication workers. We should be able to test traversing to a peer
                // after a huge update as been added to the name system.
                // wait(1).await;

                // ctx.sync().await?;

                // wait(1).await;

                // let cursor = SphereCursor::latest(ctx);
                // let _peer2_ctx = cursor
                //     .traverse_by_petnames(&["peer2".into()])
                //     .await?
                //     .unwrap();
                Ok(())
            })
            .await?;

        ns_task.abort();

        Ok(())
    }
}

mod multiplayer {
    // TODO(#629): Remove this when we migrate off of `release-please`
    extern crate noosphere_cli_dev as noosphere_cli;

    use anyhow::Result;
    use noosphere_cli::helpers::{start_name_system_server, CliSimulator, SpherePair};
    use noosphere_common::helpers::wait;
    use noosphere_core::{
        context::{
            HasMutableSphereContext, SphereAuthorityWrite, SphereContentWrite, SpherePetnameWrite,
            SphereSync,
        },
        data::Did,
        tracing::initialize_tracing,
    };
    use serde_json::Value;
    use url::Url;

    #[cfg(not(feature = "rocksdb"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn orb_can_render_peers_in_the_sphere_address_book() -> Result<()> {
        initialize_tracing(None);

        let ipfs_url = Url::parse("http://127.0.0.1:5001").unwrap();
        let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await.unwrap();

        let mut pair_1 = SpherePair::new("ONE", &ipfs_url, &ns_url).await?;
        let mut pair_2 = SpherePair::new("TWO", &ipfs_url, &ns_url).await?;
        let mut pair_3 = SpherePair::new("THREE", &ipfs_url, &ns_url).await?;
        let mut pair_4 = SpherePair::new("FOUR", &ipfs_url, &ns_url).await?;
        let mut pair_5 = SpherePair::new("FIVE", &ipfs_url, &ns_url).await?;

        pair_1.start_gateway().await?;
        pair_2.start_gateway().await?;
        pair_3.start_gateway().await?;
        pair_4.start_gateway().await?;
        pair_5.start_gateway().await?;

        let sphere_1_id = pair_1.client.identity.clone();
        let sphere_2_id = pair_2.client.identity.clone();
        let sphere_3_id = pair_3.client.identity.clone();
        let sphere_4_id = pair_4.client.identity.clone();
        let sphere_5_id = pair_5.client.identity.clone();

        for (index, pair) in [&pair_1, &pair_2, &pair_3, &pair_4, &pair_5]
            .iter()
            .enumerate()
        {
            pair.spawn(move |mut ctx| async move {
                let id = index + 1;
                ctx.write(
                    format!("content{}", id).as_str(),
                    "text/plain",
                    format!("foo{}", id).as_bytes(),
                    None,
                )
                .await?;

                ctx.save(None).await?;
                ctx.sync().await?;

                Ok(())
            })
            .await?;
        }

        {
            let sphere_3_id = sphere_3_id.clone();
            let sphere_4_id = sphere_4_id.clone();
            pair_2
                .spawn(move |mut ctx| async move {
                    ctx.set_petname("peer3-of-peer2", Some(sphere_3_id)).await?;
                    ctx.set_petname("peer4", Some(sphere_4_id)).await?;
                    ctx.save(None).await?;
                    ctx.sync().await?;
                    wait(1).await;
                    ctx.sync().await?;
                    Ok(())
                })
                .await?;
        }

        {
            let sphere_2_id = sphere_2_id.clone();
            let sphere_3_id = sphere_3_id.clone();
            pair_1
                .spawn(move |mut ctx| async move {
                    ctx.set_petname("peer2", Some(sphere_2_id)).await?;
                    ctx.set_petname("peer3", Some(sphere_3_id)).await?;
                    ctx.save(None).await?;
                    ctx.sync().await?;
                    wait(1).await;
                    ctx.sync().await?;
                    Ok(())
                })
                .await?;
        }

        let gateway_url = pair_1.client.workspace.gateway_url().await?;
        let cli = CliSimulator::new()?;

        cli.orb(&["key", "create", "foobar"]).await?;

        let cli_id = match serde_json::from_str(
            &cli.orb_with_output(&["key", "list", "--as-json"])
                .await?
                .join("\n"),
        )? {
            Value::Object(keys) => keys.get("foobar").unwrap().as_str().unwrap().to_owned(),
            _ => panic!(),
        };

        let (authorization, sphere_1_version) = pair_1
            .spawn(move |mut ctx| async move {
                let authorization = ctx.authorize("cli", &Did(cli_id)).await?;
                ctx.save(None).await?;
                let version = ctx.sync().await?;
                wait(1).await;
                Ok((authorization, version))
            })
            .await?;

        // Join the first sphere
        cli.orb(&[
            "sphere",
            "join",
            "--authorization",
            &authorization.to_string(),
            "--local-key",
            "foobar",
            "--gateway-url",
            &gateway_url.to_string(),
            &sphere_1_id,
        ])
        .await?;

        let expected_content = [
            ("content1.txt", "foo1"),
            ("@peer2/content2.txt", "foo2"),
            ("@peer3/content3.txt", "foo3"),
            ("@peer2/@peer3-of-peer2/content3.txt", "foo3"),
            ("@peer2/@peer4/content4.txt", "foo4"),
            (".sphere/identity", &sphere_1_id),
            (".sphere/version", &sphere_1_version.to_string()),
        ];

        for (path, content) in expected_content {
            let path = cli.sphere_directory().join(path);

            assert!(tokio::fs::try_exists(&path).await?);
            assert_eq!(&tokio::fs::read_to_string(&path).await?, content);
        }

        // Change a peer-of-my-peer
        pair_4
            .spawn(move |mut ctx| async move {
                ctx.write(
                    "content4",
                    "text/plain",
                    "foo4 and something new".as_bytes(),
                    None,
                )
                .await?;
                ctx.set_petname("peer5", Some(sphere_5_id)).await?;
                ctx.save(None).await?;
                ctx.sync().await?;
                wait(1).await;
                ctx.sync().await?;
                ctx.sync().await?;

                Ok(())
            })
            .await?;

        // Add another level of depth to the graph
        pair_3
            .spawn(move |mut ctx| async move {
                ctx.set_petname("peer4-of-peer3", Some(sphere_4_id)).await?;
                ctx.save(None).await?;
                ctx.sync().await?;
                wait(1).await;
                ctx.sync().await?;

                Ok(())
            })
            .await?;

        // Change a peer
        pair_2
            .spawn(move |mut ctx| async move {
                ctx.write("newcontent", "text/plain", "new".as_bytes(), None)
                    .await?;
                ctx.set_petname("peer4", None).await?;
                ctx.save(None).await?;
                ctx.sync().await?;
                wait(1).await;
                ctx.sync().await?;

                Ok(())
            })
            .await?;

        // Rename a peer
        let sphere_1_version = pair_1
            .spawn(move |mut ctx| async move {
                // Give the graph state the opportunity to "settle"
                wait(2).await;

                // Change up petnames a bit
                ctx.set_petname("peer3", None).await?;
                ctx.set_petname("peer2", None).await?;
                ctx.set_petname("peer2-renamed", Some(sphere_2_id)).await?;

                // Add some new content...
                ctx.write("never-seen", "text/plain", "boo".as_bytes(), None)
                    .await?;
                ctx.save(None).await?;

                // ...and remove it again:
                ctx.remove("never-seen").await?;
                ctx.save(None).await?;

                ctx.sync().await?;
                wait(2).await;
                let version = ctx.sync().await?;

                Ok(version)
            })
            .await?;

        wait(1).await;

        // Sync to get the latest remote changes
        cli.orb(&["sphere", "sync", "--auto-retry", "3"]).await?;

        let expected_content = [
            ("content1.txt", "foo1"),
            ("@peer2-renamed/content2.txt", "foo2"),
            ("@peer2-renamed/newcontent.txt", "new"),
            ("@peer2-renamed/@peer3-of-peer2/content3.txt", "foo3"),
            (
                "@peer2-renamed/@peer3-of-peer2/@peer4-of-peer3/content4.txt",
                "foo4 and something new",
            ),
            ("@peer2-renamed/@peer3-of-peer2/content3.txt", "foo3"),
            (
                "@peer2-renamed/@peer3-of-peer2/@peer4-of-peer3/content4.txt",
                "foo4 and something new",
            ),
            (".sphere/identity", &sphere_1_id),
            (".sphere/version", &sphere_1_version.to_string()),
        ];

        for (path, content) in expected_content {
            let path = cli.sphere_directory().join(path);

            assert!(
                tokio::fs::try_exists(&path).await?,
                "'{}' should exist",
                path.display()
            );
            assert_eq!(
                &tokio::fs::read_to_string(&path).await?,
                content,
                "'{}' should contain '{content}'",
                path.display()
            );
        }

        let peer_5_content_path =
            "@peer2-renamed/@peer3-of-peer2/@peer4-of-peer3/@peer5/content5.txt";

        let unexpected_content = [
            // Content added and removed remotely before local sync
            "never-seen",
            // Peer removed
            "@peer3/content3.txt",
            // Peer renamed
            "@peer2/content2.txt",
            // Peer removed
            "@peer2-renamed/@peer4/content4.txt",
            // Peer depth greater than render depth
            peer_5_content_path,
        ];

        for path in unexpected_content {
            assert!(
                !tokio::fs::try_exists(&cli.sphere_directory().join(path)).await?,
                "'{path}' should not exist"
            );
        }

        wait(1).await;

        // Sync again, but with a greater render depth
        cli.orb(&["sphere", "sync", "--auto-retry", "3", "--render-depth", "5"])
            .await?;

        // Previously omitted peer should be rendered now
        assert!(
            tokio::fs::try_exists(&cli.sphere_directory().join(peer_5_content_path)).await?,
            "'{peer_5_content_path}' should exist"
        );

        // Re-render using the original render depth
        cli.orb(&["sphere", "render", "--render-depth", "3"])
            .await?;

        assert!(
            !tokio::fs::try_exists(&cli.sphere_directory().join(peer_5_content_path)).await?,
            "'{peer_5_content_path}' should not exist"
        );

        ns_task.abort();

        Ok(())
    }
}
