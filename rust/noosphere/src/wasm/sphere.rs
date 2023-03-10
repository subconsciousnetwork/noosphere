use anyhow::Result;
use cid::Cid;
use std::sync::Arc;

use crate::{
    platform::{PlatformKeyMaterial, PlatformStorage},
    wasm::SphereFs,
};
use noosphere_sphere::{SphereContext as SphereContextImpl, SphereCursor};
use tokio::sync::Mutex;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
/// A `SphereContext` is a view into all of a sphere's data, that also
/// encapsulates handles to local storage and a user's authority relative to the
/// sphere. If a user is appropriately authorized, they may use a
/// `SphereContext` to modify a sphere. Otherwise, they may only read a sphere's
/// publicly visible content.
pub struct SphereContext {
    #[wasm_bindgen(skip)]
    pub inner: Arc<Mutex<SphereContextImpl<PlatformKeyMaterial, PlatformStorage>>>,
}

#[wasm_bindgen]
impl SphereContext {
    #[wasm_bindgen]
    /// Get a `SphereFs` that gives you access to sphere content at the latest
    /// version of the sphere.
    pub async fn fs(&self) -> Result<SphereFs, String> {
        Ok(SphereFs {
            inner: SphereCursor::latest(self.inner.clone()),
        })
    }

    #[wasm_bindgen(js_name = "fsAt")]
    /// Get a `SphereFs` that gives you access to sphere content at the version
    /// specified. The version must be a base32
    /// [CID](https://docs.ipfs.tech/concepts/content-addressing/#identifier-formats)
    /// string.
    pub async fn fs_at(&self, version: String) -> Result<SphereFs, String> {
        let cid = Cid::try_from(version).map_err(|error| format!("{:?}", error))?;

        Ok(SphereFs {
            inner: SphereCursor::mounted_at(self.inner.clone(), &cid),
        })
    }
}
