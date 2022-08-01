use anyhow::anyhow;
use async_trait::async_trait;
use axum::{
    body::{Bytes, HttpBody},
    extract::{FromRequest, RequestParts},
    BoxError,
};
use hyper::header;
use noosphere_cbor::TryDagCbor;

use super::GatewayError;

#[derive(Debug, Clone, Copy, Default)]
pub struct DagCbor<T: TryDagCbor>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for DagCbor<T>
where
    T: TryDagCbor,
    B: HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = GatewayError;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if !is_octet_stream_content_type(req) {
            return Err(anyhow!("Unexpected content type").into());
        }

        let bytes = Bytes::from_request(req)
            .await
            .map_err(|error| GatewayError::Internal(anyhow!(error)))?;

        Ok(DagCbor(
            T::try_from_dag_cbor(&bytes).map_err(|error| GatewayError::Internal(error))?,
        ))
    }
}

fn is_octet_stream_content_type<B>(req: &RequestParts<B>) -> bool {
    let content_type = if let Some(content_type) = req.headers().get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    let mime = if let Ok(mime) = content_type.parse::<mime::Mime>() {
        mime
    } else {
        return false;
    };

    mime == mime::APPLICATION_OCTET_STREAM
}
