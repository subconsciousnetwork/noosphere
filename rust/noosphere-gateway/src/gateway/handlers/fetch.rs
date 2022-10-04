use std::sync::Arc;

use axum::{extract::Query, response::IntoResponse, Extension};
use noosphere_api::data::FetchParameters;
use noosphere_storage::native::NativeStore;


pub async fn fetch_handler(
    Query(FetchParameters { sphere: _, since: _ }): Query<FetchParameters>,
    Extension(_store): Extension<Arc<NativeStore>>,
) -> impl IntoResponse {
    todo!()
}
