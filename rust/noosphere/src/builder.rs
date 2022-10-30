use anyhow::{anyhow, Result};
use noosphere_api::{
    client::Client,
    data::{FetchParameters, FetchResponse},
};
use noosphere_core::{authority::Authorization, view::Sphere};
use noosphere_fs::SphereFs;
use noosphere_storage::{db::SphereDb, interface::Store};
use ucan::crypto::KeyMaterial;




// pub struct NoosphereBuilder<'a, K, S> {
//   user_key: Option<K>
// }
