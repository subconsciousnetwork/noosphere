use async_trait::async_trait;
use axum::{
    body::{Bytes, HttpBody},
    extract::{FromRequest, RequestParts},
    http::{header, StatusCode},
    BoxError,
};
use libipld_cbor::DagCborCodec;
use mime_guess::mime;
use noosphere_storage::encoding::block_deserialize;
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug, Clone, Copy, Default)]
pub struct Cbor<T: Serialize + DeserializeOwned>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Cbor<T>
where
    T: Serialize + DeserializeOwned,
    B: HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if !is_octet_stream_content_type(req) {
            return Err(StatusCode::BAD_REQUEST);
        }

        let bytes = Bytes::from_request(req).await.map_err(|error| {
            error!("{:?}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Ok(Cbor(block_deserialize::<DagCborCodec, T>(&bytes).map_err(
            |error| {
                error!("{:?}", error);
                StatusCode::BAD_REQUEST
            },
        )?))
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
