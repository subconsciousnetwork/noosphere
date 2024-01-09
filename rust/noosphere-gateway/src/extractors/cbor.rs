use async_trait::async_trait;
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::{header, StatusCode},
    response::IntoResponse,
};
use libipld_cbor::DagCborCodec;
use mime_guess::mime;
use noosphere_storage::{block_deserialize, block_serialize};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug, Clone, Copy, Default)]
pub struct Cbor<T: Serialize + DeserializeOwned>(pub T);

impl<T> IntoResponse for Cbor<T>
where
    T: Serialize + DeserializeOwned,
{
    fn into_response(self) -> axum::response::Response {
        match block_serialize::<DagCborCodec, _>(self.0) {
            Ok((_, bytes)) => bytes.into_response(),
            Err(error) => {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[async_trait]
impl<S, T> FromRequest<S> for Cbor<T>
where
    T: Serialize + DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if !is_octet_stream_content_type(&req) {
            return Err(StatusCode::BAD_REQUEST);
        }

        let bytes = Bytes::from_request(req, state).await.map_err(|error| {
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

fn is_octet_stream_content_type(req: &Request) -> bool {
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
