// Adapted for Noosphere from https://github.com/filecoin-project/ref-fvm
// Source copyright and license:
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::hamt::Hamt;
use forest_hash_utils::BytesKey;
use serde_bytes::ByteBuf;

use noosphere_storage::{MemoryStore, StoreStats, TrackingStore};

use tokio_stream::StreamExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

// Redeclaring max array size of Hamt to avoid exposing value
const BUCKET_SIZE: usize = 3;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn test_basics() {
    let store = MemoryStore::default();
    let mut hamt = Hamt::<_, String, _>::new(store);
    hamt.set(1, "world".to_string()).await.unwrap();

    assert_eq!(hamt.get(&1).await.unwrap(), Some(&"world".to_string()));
    hamt.set(1, "world2".to_string()).await.unwrap();
    assert_eq!(hamt.get(&1).await.unwrap(), Some(&"world2".to_string()));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn test_load() {
    let store = MemoryStore::default();

    let mut hamt: Hamt<_, _, usize> = Hamt::new(store.clone());
    hamt.set(1, "world".to_string()).await.unwrap();

    assert_eq!(hamt.get(&1).await.unwrap(), Some(&"world".to_string()));
    hamt.set(1, "world2".to_string()).await.unwrap();
    assert_eq!(hamt.get(&1).await.unwrap(), Some(&"world2".to_string()));
    let c = hamt.flush().await.unwrap();

    let new_hamt = Hamt::load(&c, store.clone()).await.unwrap();
    assert_eq!(hamt, new_hamt);

    // set value in the first one
    hamt.set(2, "stuff".to_string()).await.unwrap();

    // loading original hash should returnnot be equal now
    let new_hamt = Hamt::load(&c, store.clone()).await.unwrap();
    assert_ne!(hamt, new_hamt);

    // loading new hash
    let c2 = hamt.flush().await.unwrap();
    let new_hamt = Hamt::load(&c2, store.clone()).await.unwrap();
    assert_eq!(hamt, new_hamt);

    // loading from an empty store does not work
    let empty_store = MemoryStore::default();
    assert!(Hamt::<_, String, usize>::load(&c2, empty_store)
        .await
        .is_err());

    // storing the hamt should produce the same cid as storing the root
    let c3 = hamt.flush().await.unwrap();
    assert_eq!(c3, c2);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn test_set_if_absent() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut hamt: Hamt<_, _, _> = Hamt::new(store.clone());
    assert!(hamt
        .set_if_absent("favorite-animal".to_string(), "owl bear".to_string())
        .await
        .unwrap());

    // Next two are negatively asserted, shouldn't change
    assert!(!hamt
        .set_if_absent(
            "favorite-animal".to_string(),
            "bright green bear".to_string()
        )
        .await
        .unwrap());
    assert!(!hamt
        .set_if_absent("favorite-animal".to_string(), "owl bear".to_string())
        .await
        .unwrap());

    let c = hamt.flush().await.unwrap();

    let mut h2 = Hamt::<_, _, _>::load(&c, store.clone()).await.unwrap();
    // Reloading should still have same effect
    assert!(!h2
        .set_if_absent(
            "favorite-animal".to_string(),
            "bright green bear".to_string()
        )
        .await
        .unwrap());

    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzacebepgau47qooinjprs6askm2ogbkqpx5nn7ixb5upo36k6qfevinu"
    );
    let stats = store.to_stats().await;

    assert_eq!(
        stats,
        StoreStats {
            reads: 1,
            writes: 2,
            removes: 0,
            bytes_read: 106,
            bytes_written: 115,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn set_with_no_effect_does_not_put() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut begn: Hamt<_, _, _> = Hamt::new_with_bit_width(store.clone(), 1);
    let entries = 2 * BUCKET_SIZE * 5;
    for i in 0..entries {
        begn.set(i.to_string(), "filler".to_string()).await.unwrap();
    }

    let c = begn.flush().await.unwrap();
    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzacedmaokbwzqptq254dn32xtg7solqwgksyhldyw2igup2haq5kticg"
    );

    begn.set(
        "favorite-animal".to_string(),
        "bright green bear".to_string(),
    )
    .await
    .unwrap();
    let c2 = begn.flush().await.unwrap();
    assert_eq!(
        c2.to_string().as_str(),
        "bafy2bzacedupopofs73ow7txfhy3ishm7ikwt5vw6mkjmp35fjfzzdwrd3kog"
    );
    let stats = store.to_stats().await;

    assert_eq!(
        stats,
        StoreStats {
            reads: 0,
            writes: 88,
            removes: 0,
            bytes_read: 0,
            bytes_written: 3437,
            bytes_removed: 0,
            flushes: 0
        }
    );

    begn.set(
        "favorite-animal".to_string(),
        "bright green bear".to_string(),
    )
    .await
    .unwrap();

    let c3 = begn.flush().await.unwrap();
    assert_eq!(
        c3.to_string().as_str(),
        "bafy2bzacedupopofs73ow7txfhy3ishm7ikwt5vw6mkjmp35fjfzzdwrd3kog"
    );

    let stats = store.to_stats().await;

    assert_eq!(
        stats,
        StoreStats {
            reads: 0,
            writes: 90,
            removes: 0,
            bytes_read: 0,
            bytes_written: 3545,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn delete() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut hamt: Hamt<_, _, _> = Hamt::new(store.clone());
    hamt.set("foo".to_string(), "cat dog bear".to_string())
        .await
        .unwrap();
    hamt.set("bar".to_string(), "cat dog".to_string())
        .await
        .unwrap();
    hamt.set("baz".to_string(), "cat".to_string())
        .await
        .unwrap();

    let c = hamt.flush().await.unwrap();
    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzacedtdpjgi2d3wafmylxreii5xtx63u6thez44f2mvqplo345vtttiq"
    );

    let mut h2 = Hamt::<_, String, String>::load(&c, store.clone())
        .await
        .unwrap();
    assert!(h2.delete(&"foo".to_string()).await.unwrap().is_some());
    assert_eq!(h2.get(&"foo".to_string()).await.unwrap(), None);

    let c2 = h2.flush().await.unwrap();
    assert_eq!(
        c2.to_string().as_str(),
        "bafy2bzaceboyrlvn3q6enubhyuyojeup3iqtlghbr7b3o25aeyzvzqwniw7xo"
    );
    let stats = store.to_stats().await;
    assert_eq!(
        stats,
        StoreStats {
            reads: 2,
            writes: 5,
            removes: 0,
            bytes_read: 223,
            bytes_written: 387,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn delete_case() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut hamt: Hamt<_, _, _> = Hamt::new(store.clone());

    hamt.set([0u8].to_vec(), ByteBuf::from(b"Test data".as_ref()))
        .await
        .unwrap();

    let c = hamt.flush().await.unwrap();
    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzaceall47zxpihq2cm6jweri2rfi5wr2qtfww3wzaph2j63ah7mf7ppy"
    );

    let mut h2: Hamt<_, ByteBuf, Vec<u8>> = Hamt::load(&c, store.clone()).await.unwrap();
    assert!(h2.delete(&[0u8].to_vec()).await.unwrap().is_some());
    assert_eq!(h2.get(&[0u8].to_vec()).await.unwrap(), None);

    let c2 = h2.flush().await.unwrap();
    assert_eq!(
        c2.to_string().as_str(),
        "bafy2bzaceamp42wmmgr2g2ymg46euououzfyck7szknvfacqscohrvaikwfay"
    );
    let stats = store.to_stats().await;
    assert_eq!(
        stats,
        StoreStats {
            reads: 2,
            writes: 3,
            removes: 0,
            bytes_read: 83,
            bytes_written: 86,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn reload_empty() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut hamt: Hamt<_, (), ()> = Hamt::new(store.clone());

    let c = hamt.flush().await.unwrap();

    let mut h2 = Hamt::<_, (), ()>::load(&c, store.clone()).await.unwrap();

    let c2 = h2.flush().await.unwrap();

    assert_eq!(c, c2);
    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzaceamp42wmmgr2g2ymg46euououzfyck7szknvfacqscohrvaikwfay"
    );
    #[rustfmt::skip]
    let stats = store.to_stats().await;

    assert_eq!(
        stats,
        StoreStats {
            reads: 1,
            writes: 2,
            removes: 0,
            bytes_read: 3,
            bytes_written: 6,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn set_delete_many() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    // Test vectors setup specifically for bit width of 5
    let mut hamt: Hamt<_, i32, i32> = Hamt::new_with_bit_width(store.clone(), 5);

    for i in 0..200 {
        hamt.set(i, i).await.unwrap();
    }

    let c1 = hamt.flush().await.unwrap();
    assert_eq!(
        c1.to_string().as_str(),
        "bafy2bzacebz6wrw6qbo5ylu6lhnpbjvph6wo7x3zo2yr6vx3c3hi4vqhqm5jo"
    );

    for i in 200..400 {
        hamt.set(i, i).await.unwrap();
    }

    let cid_all = hamt.flush().await.unwrap();
    assert_eq!(
        cid_all.to_string().as_str(),
        "bafy2bzacebzrkxafuqv6lh7asongt3kfw6zzuvfgwqtsholvuwuhwhlfrj4kq"
    );

    for i in 200..400 {
        assert!(hamt.delete(&i).await.unwrap().is_some());
    }
    // Ensure first 200 keys still exist
    for i in 0..200 {
        assert_eq!(hamt.get(&i).await.unwrap(), Some(&i));
    }

    let cid_d = hamt.flush().await.unwrap();
    assert_eq!(
        cid_d.to_string().as_str(),
        "bafy2bzacebz6wrw6qbo5ylu6lhnpbjvph6wo7x3zo2yr6vx3c3hi4vqhqm5jo"
    );

    let stats = store.to_stats().await;
    assert_eq!(
        stats,
        StoreStats {
            reads: 0,
            writes: 587,
            removes: 0,
            bytes_read: 0,
            bytes_written: 50268,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn into_stream() {
    let mem = MemoryStore::default();

    let mut hamt: Hamt<_, i32, i32> = Hamt::new_with_bit_width(mem, 5);

    for i in 0..200 {
        hamt.set(i, i).await.unwrap();
    }

    hamt.flush().await.unwrap();

    for i in 200..400 {
        hamt.set(i, i).await.unwrap();
    }

    // Iterating through hamt with dirty caches.
    let mut count = 0;
    let stream = hamt.into_stream();
    tokio::pin!(stream);

    while let Ok(Some((k, v))) = stream.try_next().await {
        assert_eq!(k, v);
        count += 1;
    }

    assert_eq!(count, 400);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn for_each() {
    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut hamt: Hamt<_, i32, i32> = Hamt::new_with_bit_width(store.clone(), 5);

    for i in 0..200 {
        hamt.set(i, i).await.unwrap();
    }

    // Iterating through hamt with dirty caches.
    let mut count = 0;
    hamt.for_each(|k, v| {
        assert_eq!(k, v);
        count += 1;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(count, 200);

    let c = hamt.flush().await.unwrap();
    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzacebz6wrw6qbo5ylu6lhnpbjvph6wo7x3zo2yr6vx3c3hi4vqhqm5jo"
    );

    let mut hamt: Hamt<_, i32, i32> = Hamt::load_with_bit_width(&c, store.clone(), 5)
        .await
        .unwrap();

    // Iterating through hamt with no cache.
    let mut count = 0;
    hamt.for_each(|k, v| {
        assert_eq!(k, v);
        count += 1;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(count, 200);

    // Iterating through hamt with cached nodes.
    let mut count = 0;
    hamt.for_each(|k, v| {
        assert_eq!(k, v);
        count += 1;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(count, 200);

    let c = hamt.flush().await.unwrap();
    assert_eq!(
        c.to_string().as_str(),
        "bafy2bzacebz6wrw6qbo5ylu6lhnpbjvph6wo7x3zo2yr6vx3c3hi4vqhqm5jo"
    );

    let stats = store.to_stats().await;

    assert_eq!(
        stats,
        StoreStats {
            reads: 229,
            writes: 314,
            removes: 0,
            bytes_read: 12936,
            bytes_written: 14902,
            bytes_removed: 0,
            flushes: 0
        }
    );
}

#[cfg(feature = "identity")]
use crate::hamt::Identity;

#[cfg(feature = "identity")]
async fn add_and_remove_keys<'a>(
    bit_width: u32,
    keys: &'a [&'a [u8]],
    extra_keys: &'a [&'a [u8]],
    expected: &'static str,
    stats: StoreStats,
) -> () {
    let all: Vec<(BytesKey, BytesKey)> = keys
        .iter()
        .enumerate()
        // Value doesn't matter for this test, only checking cids against previous
        .map(|(i, k)| (k.to_vec().into(), tstring(i)))
        .collect();

    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut hamt: Hamt<_, _, _, Identity> = Hamt::new_with_bit_width(store.clone(), bit_width);

    for (k, v) in all.iter() {
        hamt.set(k.clone(), v.clone()).await.unwrap();
    }
    let cid = hamt.flush().await.unwrap();

    let mut h1: Hamt<_, _, BytesKey, Identity> =
        Hamt::load_with_bit_width(&cid, store.clone(), bit_width)
            .await
            .unwrap();

    for (k, v) in all {
        assert_eq!(Some(&v), h1.get(&k).await.unwrap());
    }

    // Set and delete extra keys
    for k in extra_keys.iter() {
        hamt.set(k.to_vec().into(), tstring(0)).await.unwrap();
    }
    for k in extra_keys.iter() {
        hamt.delete(*k).await.unwrap();
    }
    let cid2 = hamt.flush().await.unwrap();
    let mut h2: Hamt<_, BytesKey, BytesKey, Identity> =
        Hamt::load_with_bit_width(&cid2, store.clone(), bit_width)
            .await
            .unwrap();

    let cid1 = h1.flush().await.unwrap();
    let cid2 = h2.flush().await.unwrap();
    assert_eq!(cid1, cid2);
    assert_eq!(cid1.to_string().as_str(), expected);

    let new_stats = store.to_stats().await;

    assert_eq!(new_stats, stats);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg(feature = "identity")]
async fn canonical_structure() {
    // Champ mutation semantics test
    add_and_remove_keys(
        8,
        &[b"K"],
        &[b"B"],
        "bafy2bzacecosy45hp4sz2t4o4flxvntnwjy7yaq43bykci22xycpeuj542lse",
        StoreStats {
            reads: 6,
            writes: 1,
            removes: 0,
            bytes_read: 38,
            bytes_written: 19,
            bytes_removed: 0,
            flushes: 0,
        },
    )
    .await;

    add_and_remove_keys(
        8,
        &[b"K0", b"K1", b"KAA1", b"KAA2", b"KAA3"],
        &[b"KAA4"],
        "bafy2bzaceaqdaj5aqkwugr7wx4to3fahynoqlxuo5j6xznly3khazgyxihkbo",
        StoreStats {
            reads: 9,
            writes: 2,
            removes: 0,
            bytes_read: 163,
            bytes_written: 107,
            bytes_removed: 0,
            flushes: 0,
        },
    )
    .await;
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg(feature = "identity")]
async fn canonical_structure_alt_bit_width() {
    let kb_cases = [
        "bafy2bzacec3cquclaqkb32cntwtizgij55b7isb4s5hv5hv5ujbbeu6clxkug",
        "bafy2bzacebj7b2jahw7nxmu6mlhkwzucjmfq7aqlj52jusqtufqtaxcma4pdm",
        "bafy2bzacedrwwndijql6lmmtyicjwyehxtgey5fhzocc43hrzhetrz25v2k2y",
    ];

    let other_cases = [
        "bafy2bzacedbiipe7l7gbtjandyyl6rqlkuqr2im2nl7d4bljidv5mta22rjqk",
        "bafy2bzaceb3c76qlbsiv3baogpao3zah56eqonsowpkof33o5hmncfow4seso",
        "bafy2bzacebhkyrwfexokaoygsx2crydq3fosiyfoa5bthphntmicsco2xf442",
    ];

    let kb_stats = [
        StoreStats {
            reads: 6,
            writes: 1,
            removes: 0,
            bytes_read: 22,
            bytes_written: 11,
            bytes_removed: 0,
            flushes: 0,
        },
        StoreStats {
            reads: 6,
            writes: 1,
            removes: 0,
            bytes_read: 24,
            bytes_written: 12,
            bytes_removed: 0,
            flushes: 0,
        },
        StoreStats {
            reads: 6,
            writes: 1,
            removes: 0,
            bytes_read: 28,
            bytes_written: 14,
            bytes_removed: 0,
            flushes: 0,
        },
    ];

    let other_stats = [
        StoreStats {
            reads: 9,
            writes: 2,
            removes: 0,
            bytes_read: 139,
            bytes_written: 91,
            bytes_removed: 0,
            flushes: 0,
        },
        StoreStats {
            reads: 9,
            writes: 2,
            removes: 0,
            bytes_read: 146,
            bytes_written: 97,
            bytes_removed: 0,
            flushes: 0,
        },
        StoreStats {
            reads: 9,
            writes: 2,
            removes: 0,
            bytes_read: 154,
            bytes_written: 103,
            bytes_removed: 0,
            flushes: 0,
        },
    ];

    for i in 5..8 {
        add_and_remove_keys(
            i,
            &[b"K"],
            &[b"B"],
            kb_cases[(i - 5) as usize],
            kb_stats[(i - 5) as usize].clone(),
        )
        .await;
        add_and_remove_keys(
            i,
            &[b"K0", b"K1", b"KAA1", b"KAA2", b"KAA3"],
            &[b"KAA4"],
            other_cases[(i - 5) as usize],
            other_stats[(i - 5) as usize].clone(),
        )
        .await;
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn clean_child_ordering() {
    let make_key = |i: u64| -> BytesKey {
        let mut key = unsigned_varint::encode::u64_buffer();
        let n = unsigned_varint::encode::u64(i, &mut key);
        n.to_vec().into()
    };

    let dummy_value: u8 = 42;

    let mem = MemoryStore::default();
    let store = TrackingStore::wrap(mem);

    let mut h: Hamt<_, _> = Hamt::new_with_bit_width(store.clone(), 5);

    for i in 100..195 {
        h.set(make_key(i), dummy_value).await.unwrap();
    }

    let root = h.flush().await.unwrap();
    assert_eq!(
        root.to_string().as_str(),
        "bafy2bzacedg3vblqt43unwxqa6atscg6awq6uarmdlao74j3mxzmagc4t73mk"
    );
    let mut h = Hamt::<_, u8>::load_with_bit_width(&root, store.clone(), 5)
        .await
        .unwrap();

    h.delete(&make_key(104)).await.unwrap();
    h.delete(&make_key(108)).await.unwrap();
    let root = h.flush().await.unwrap();
    Hamt::<_, u8>::load_with_bit_width(&root, store.clone(), 5)
        .await
        .unwrap();

    assert_eq!(
        root.to_string().as_str(),
        "bafy2bzaced4pvk3pvchjlyguc5wdsbnk3nfntdxgx3dz4ko2snqipro7f7bl2"
    );

    let stats = store.to_stats().await;

    assert_eq!(
        stats,
        StoreStats {
            reads: 5,
            writes: 133,
            removes: 0,
            bytes_read: 7153,
            bytes_written: 9545,
            bytes_removed: 0,
            flushes: 0
        }
    );
}
