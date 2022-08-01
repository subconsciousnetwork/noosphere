use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub trait TryDagCborConditionalSendSync: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait TryDagCborConditionalSendSync {}

pub trait TryDagCbor: Serialize + DeserializeOwned {
    fn try_into_dag_cbor(&self) -> Result<Vec<u8>> {
        Ok(serde_ipld_dagcbor::to_vec(self)?)
    }

    fn try_from_dag_cbor(dag_cbor: &[u8]) -> Result<Self> {
        Ok(serde_ipld_dagcbor::from_slice(dag_cbor)?)
    }
}

pub trait TryDagCborSendSync: TryDagCbor + TryDagCborConditionalSendSync {}

impl<T> TryDagCbor for T where T: Serialize + DeserializeOwned {}

impl<T> TryDagCborSendSync for T where
    T: Serialize + DeserializeOwned + TryDagCborConditionalSendSync
{
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> TryDagCborConditionalSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
impl<T> TryDagCborConditionalSendSync for T {}
