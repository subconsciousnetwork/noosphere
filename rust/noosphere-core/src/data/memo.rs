use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use serde::{Deserialize, Serialize};
use ucan::{crypto::KeyMaterial, Ucan};

use crate::data::Header;

use noosphere_storage::{base64_encode, BlockStore, BlockStoreSend};

use super::{ContentType, Link};

pub type LamportOrder = u32;

/// A basic Memo. A Memo is a history-retaining structure that pairs
/// inline headers with a body CID.
#[derive(Debug, Default, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct MemoIpld {
    /// An optional pointer to the previous version of the DAG
    pub parent: Option<Link<MemoIpld>>,
    /// Headers that are associated with the content of this DAG
    pub headers: Vec<(String, String)>,
    /// A pointer to the body content
    pub body: Cid,
}

impl MemoIpld {
    /// Gets the Lamport order from the memo's headers, if one is set. Returns
    /// the starting order value if no such header is found.
    pub fn lamport_order(&self) -> LamportOrder {
        if let Some(lamport_order) = self.get_first_header(&Header::LamportOrder) {
            u32::from_str(&lamport_order).unwrap_or_default()
        } else {
            0u32
        }
    }

    /// If there is a "proof" header in the memo, attempts to interpret it as a
    /// [Ucan] and return the result.
    pub fn get_proof(&self) -> Result<Option<Ucan>> {
        if let Some(since_proof) = self.get_first_header(&Header::Proof) {
            if let Ok(ucan) = Ucan::from_str(&since_proof) {
                return Ok(Some(ucan));
            }
        };
        Ok(None)
    }

    /// Same as [Ucan::get_proof] except it returns an error result if a valid
    /// "proof" header is not found.
    pub fn require_proof(&self) -> Result<Ucan> {
        if let Some(ucan) = self.get_proof()? {
            Ok(ucan)
        } else {
            Err(anyhow!("No valid 'proof' header found"))
        }
    }

    /// If the body of this memo is different from it's parent, returns true.
    pub async fn compare_body<S: BlockStore>(&self, store: &S) -> Result<bool> {
        let parent_cid = match &self.parent {
            Some(cid) => cid,
            None => return Ok(true),
        };

        let MemoIpld {
            body: parent_body, ..
        } = store.load::<DagCborCodec, _>(&parent_cid).await?;

        Ok(self.body != parent_body)
    }

    /// Get the list of headers that either do not appear in other, or
    /// have a different value from a same-named header in other
    pub async fn diff_headers(&self, other: &MemoIpld) -> Result<Vec<(String, String)>> {
        let headers: BTreeMap<String, String> = self.headers.clone().into_iter().collect();
        let other_headers: BTreeMap<String, String> = other.headers.clone().into_iter().collect();

        let mut diff = Vec::new();

        for (name, value) in headers {
            if let Some(other_value) = other_headers.get(&name) {
                if value != *other_value {
                    diff.push((name, value))
                }
            } else {
                diff.push((name, value))
            }
        }

        Ok(diff)
    }

    /// Initializes a memo for the provided body, persisting the body to storage
    /// and returning the memo. Note that only the body is persisted, not the
    /// memo that wraps it.
    pub async fn for_body<S: BlockStore, Body: Serialize + BlockStoreSend>(
        store: &mut S,
        body: Body,
    ) -> Result<MemoIpld> {
        let body_cid = store.save::<DagCborCodec, _>(body).await?;
        Ok(MemoIpld {
            parent: None,
            headers: Vec::new(),
            body: body_cid,
        })
    }

    /// Loads a memo from the provided CID, initializes a copy of it, sets the
    /// copy's parent to the provided CID and cleans signature information from
    /// the copy's headers, increments the Lamport order of the memo; the new
    /// memo is returned.
    pub async fn branch_from<S: BlockStore>(cid: &Link<MemoIpld>, store: &S) -> Result<Self> {
        match store.load::<DagCborCodec, MemoIpld>(cid).await {
            Ok(mut memo) => {
                memo.parent = Some(cid.clone());
                memo.remove_header(&Header::Signature.to_string());
                memo.remove_header(&Header::Proof.to_string());

                memo.replace_headers(vec![(
                    Header::LamportOrder.to_string(),
                    (memo.lamport_order() + 1).to_string(),
                )]);

                Ok(memo)
            }
            Err(error) => Err(anyhow!(error)),
        }
    }

    /// Sign the memo's body CID, adding the signature and proof as headers in
    /// the memo
    pub async fn sign<Credential: KeyMaterial>(
        &mut self,
        credential: &Credential,
        proof: Option<&Ucan>,
    ) -> Result<()> {
        let signature = base64_encode(&credential.sign(&self.body.to_bytes()).await?)?;

        self.replace_first_header(&Header::Signature.to_string(), &signature);

        if let Some(proof) = proof {
            self.replace_first_header(&Header::Proof.to_string(), &proof.encode()?);
        } else {
            self.remove_header(&Header::Proof.to_string())
        }

        let did = credential.get_did().await?;

        self.replace_first_header(&Header::Author.to_string(), &did);

        Ok(())
    }

    /// Retreive the set of headers that matches the given string name
    pub fn get_header(&self, name: &str) -> Vec<String> {
        let lower_name = name.to_lowercase();

        self.headers
            .iter()
            .filter_map(|(a_name, a_value)| {
                if a_name.to_lowercase() == lower_name {
                    Some(a_value.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Retrieve the first header value (if any) that matches the given header
    /// name
    pub fn get_first_header(&self, name: &str) -> Option<String> {
        let lower_name = name.to_lowercase();

        for (a_name, a_value) in &self.headers {
            if a_name.to_lowercase() == lower_name {
                return Some(a_value.clone());
            }
        }
        None
    }

    /// Asserts that a header with the given name and value exists in the memo
    pub fn expect_header(&self, name: &str, value: &str) -> Result<()> {
        let lower_name = name.to_lowercase();

        for (a_name, a_value) in self.headers.iter() {
            if a_name.to_lowercase() == lower_name && a_value == value {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Expected to find a header {:?} that is {:?}",
            name,
            value
        ))
    }

    /// Replaces the value of the first header that matches name with provided
    /// value
    pub fn replace_first_header(&mut self, name: &str, value: &str) {
        let mut found = 0usize;

        self.headers = self
            .headers
            .clone()
            .into_iter()
            .filter_map(|(a_name, a_value)| {
                if a_name.to_lowercase() == name.to_lowercase() {
                    let replacement = if found == 0 {
                        Some((name.to_string(), value.to_string()))
                    } else {
                        None
                    };

                    found += 1;
                    replacement
                } else {
                    Some((a_name, a_value))
                }
            })
            .collect();

        if found == 0 {
            self.headers.push((name.to_string(), value.to_string()))
        }
    }

    /// Replaces all headers in the memo whose names match names that occur in the input
    /// list of headers. If multiple headers with the same name already occur in the memo,
    /// all of them will be removed. If multiple headers with the same name already occur
    /// in the input list, all of them will be included in the new set of headers.
    pub fn replace_headers(&mut self, mut new_headers: Vec<(String, String)>) {
        let new_header_set = new_headers
            .iter()
            .fold(BTreeSet::new(), |mut set, (key, _)| {
                set.insert(key.to_lowercase());
                set
            });

        let mut modified_headers: Vec<(String, String)> = self
            .headers
            .clone()
            .into_iter()
            .filter(|(key, _)| !new_header_set.contains(&key.to_lowercase()))
            .collect();

        modified_headers.append(&mut new_headers);

        self.headers = modified_headers;
    }

    /// Removes all headers with the given name from the memo
    pub fn remove_header(&mut self, name: &str) {
        let lower_name = name.to_lowercase();

        self.headers = self
            .headers
            .clone()
            .into_iter()
            .filter(|(a_name, _)| a_name.to_lowercase() != lower_name)
            .collect();
    }

    /// Helper to quickly deserialize a content-type (if any) from the memo
    pub fn content_type(&self) -> Option<ContentType> {
        if let Some(content_type) = self.get_first_header(&Header::ContentType.to_string()) {
            if let Ok(content_type) = ContentType::from_str(&content_type) {
                Some(content_type)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use libipld_cbor::DagCborCodec;
    use libipld_core::{ipld::Ipld, raw::RawCodec};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use serde::{Deserialize, Serialize};

    use crate::data::MemoIpld;

    use noosphere_storage::{
        block_deserialize, block_encode, block_serialize, BlockStore, MemoryStore,
    };

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_round_trips_as_cbor() {
        let (body_cid, _) = block_encode::<RawCodec, _>(&Ipld::Bytes(b"foobar".to_vec())).unwrap();
        let mut store = MemoryStore::default();

        let memo = MemoIpld {
            parent: None,
            headers: Vec::new(),
            body: body_cid,
        };

        let memo_cid = store.save::<DagCborCodec, _>(&memo).await.unwrap();
        let loaded_memo = store
            .load::<DagCborCodec, MemoIpld>(&memo_cid)
            .await
            .unwrap();

        assert_eq!(memo, loaded_memo);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_store_and_load_a_structured_body() {
        #[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
        struct Structured {
            foo: String,
        }

        let mut store = MemoryStore::default();

        let structured = Structured {
            foo: String::from("bar"),
        };
        let body_cid = store.save::<DagCborCodec, _>(&structured).await.unwrap();
        let memo = MemoIpld {
            parent: None,
            headers: Vec::new(),
            body: body_cid,
        };
        let (_, memo_bytes) = block_serialize::<DagCborCodec, _>(&memo).unwrap();
        let decoded_memo = block_deserialize::<DagCborCodec, MemoIpld>(&memo_bytes).unwrap();
        let decoded_body: Structured = store
            .load::<DagCborCodec, Structured>(&decoded_memo.body)
            .await
            .unwrap();

        assert_eq!(decoded_body.foo, String::from("bar"));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_causally_order_a_lineage_of_memos() -> Result<()> {
        let mut store = MemoryStore::default();

        let mut memo = MemoIpld::default();
        let mut memo_cid = store.save::<DagCborCodec, _>(&memo).await?;

        let mut memos = vec![memo];

        for _ in 0..5 {
            memo = MemoIpld::branch_from(&memo_cid.into(), &mut store).await?;
            memo_cid = store.save::<DagCborCodec, _>(&memo).await?;

            memos.push(memo);
        }

        let first_memo = memos.get(0).unwrap();
        let third_memo = memos.get(2).unwrap();
        let fourth_memo = memos.get(3).unwrap();
        let fifth_memo = memos.get(4).unwrap();

        assert!(third_memo.lamport_order() < fourth_memo.lamport_order());
        assert!(fifth_memo.lamport_order() > fourth_memo.lamport_order());
        assert!(first_memo.lamport_order() < third_memo.lamport_order());
        assert!(first_memo.lamport_order() < third_memo.lamport_order());

        Ok(())
    }
}
