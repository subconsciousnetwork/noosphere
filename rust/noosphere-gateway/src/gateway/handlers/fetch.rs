use std::sync::Arc;

use axum::{extract::Query, response::IntoResponse, Extension};
use noosphere_api::data::FetchParameters;
use noosphere_storage::native::NativeStore;
use serde::Deserialize;

pub async fn fetch_handler(
    Query(FetchParameters { sphere, since }): Query<FetchParameters>,
    Extension(store): Extension<Arc<NativeStore>>,
) -> impl IntoResponse {
    todo!()
}
