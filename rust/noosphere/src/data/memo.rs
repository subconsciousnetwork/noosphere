use std::{collections::BTreeMap, str::FromStr};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use serde::{Deserialize, Serialize};
use ucan::{crypto::KeyMaterial, ucan::Ucan};

use crate::{data::Header, encoding::base64_encode};

use noosphere_storage::interface::{BlockStore, BlockStoreSend};

use super::ContentType;

/// A basic Memo. A Memo is a history-retaining structure that pairs
/// inline headers with a body CID.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct MemoIpld {
    /// An optional pointer to the previous version of the DAG
    pub parent: Option<Cid>,
    /// Headers that are associated with the content of this DAG
    pub headers: Vec<(String, String)>,
    /// A pointer to the body content
    pub body: Cid,
}

impl MemoIpld {
    /// If the body of this memo is different from it's parent, returns true.
    pub async fn try_compare_body<S: BlockStore>(&self, store: &S) -> Result<bool> {
        let parent_cid = match self.parent {
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

    /// Loads a memo from the provided CID, initializes a copy of it, sets
    /// the copy's parent to the provided CID and cleans signature information
    /// from the copy's headers; the new memo is returned.
    pub async fn branch_from<S: BlockStore>(cid: &Cid, store: &S) -> Result<Self> {
        match store.load::<DagCborCodec, MemoIpld>(cid).await {
            Ok(mut memo) => {
                memo.parent = Some(*cid);
                memo.remove_header(&Header::Signature.to_string());
                memo.remove_header(&Header::Proof.to_string());

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

        self.replace_header(&Header::Signature.to_string(), &signature);

        if let Some(proof) = proof {
            self.replace_header(&Header::Proof.to_string(), &proof.encode()?);
        } else {
            self.remove_header(&Header::Proof.to_string())
        }

        let did = credential.get_did().await?;

        self.replace_header(&Header::Author.to_string(), &did);

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

    /// Retrieve the first header (if any) that matches the given string name
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
    pub fn replace_header(&mut self, name: &str, value: &str) {
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
    use libipld_cbor::DagCborCodec;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use cid::{
        multihash::{Code, MultihashDigest},
        Cid,
    };
    use serde::{Deserialize, Serialize};

    pub fn make_raw_cid<B: AsRef<[u8]>>(bytes: B) -> Cid {
        Cid::new_v1(0x55, Code::Blake2b256.digest(bytes.as_ref()))
    }

    use crate::{
        data::MemoIpld,
        encoding::{block_deserialize, block_serialize},
    };

    use noosphere_storage::{interface::BlockStore, memory::MemoryStore};

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_round_trips_as_cbor() {
        let body_cid = make_raw_cid(b"foobar");
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
}
