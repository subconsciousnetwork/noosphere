use std::str::FromStr;

use crate::{
    api::{route::RouteUrl, v0alpha1, v0alpha2},
    error::NoosphereError,
    stream::{from_car_stream, memo_history_stream, put_block_stream, to_car_stream},
};
use anyhow::{anyhow, Result};
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use iroh_car::CarReader;
use libipld_cbor::DagCborCodec;
use noosphere_common::{ConditionalSend, ConditionalSync, UnsharedStream};

use crate::{
    authority::{generate_capability, Author, SphereAbility, SphereReference},
    data::{Link, MemoIpld},
};
use noosphere_storage::{block_deserialize, block_serialize, BlockStore};
use reqwest::{header::HeaderMap, StatusCode};
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::StreamReader;
use tracing::*;
use ucan::{
    builder::UcanBuilder,
    capability::CapabilityView,
    crypto::{did::DidParser, KeyMaterial},
    store::{UcanJwtStore, UcanStore},
    ucan::Ucan,
};
use url::Url;

#[cfg(any(doc, feature = "test-gateway"))]
use crate::data::Did;

use super::v0alpha1::ReplicationMode;

/// A [Client] is a simple, portable HTTP client for the Noosphere gateway REST
/// API. It embodies the intended usage of the REST API, which includes an
/// opening handshake (with associated key verification) and various
/// UCAN-authorized verbs over sphere data.
///
/// When built with the `test-gateway` flag, a modified Host header
/// is sent, rewriting Host to `{IDENTITY}.gateway.test`, where IDENTITY
/// is the client sphere identity with the `did:key:` prefix stripped.
pub struct Client<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: UcanStore + BlockStore + 'static,
{
    /// The [v0alpha1::IdentifyResponse] that was received from the gateway when
    /// the [Client] was initialized
    pub session: v0alpha1::IdentifyResponse,

    /// The [Did] of the sphere represented by this [Client]
    pub sphere_identity: String,

    /// The [Url] for the gateway API being used by this [Client]
    pub api_base: Url,

    /// The [Author] that is wielding this [Client] to interact with the gateway API
    pub author: Author<K>,

    /// The backing [BlockStore] (also used as a [UcanStore]) for this [Client]
    pub store: S,

    client: reqwest::Client,

    #[cfg(feature = "test-gateway")]
    forced_host_header: reqwest::header::HeaderValue,
}

impl<K, S> Client<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: UcanStore + BlockStore + 'static,
{
    /// Initialize the [Client] by perfoming an "identification" handshake with
    /// a gateway whose API presumably lives at the specified URL. The request
    /// is authorized (so the provided [Author] must have the appropriate
    /// credentials), and the gateway responds with a
    /// [v0alpha1::IdentifyResponse] to verify its own credentials for the
    /// client.
    pub async fn identify(
        sphere_identity: &str,
        api_base: &Url,
        author: &Author<K>,
        did_parser: &mut DidParser,
        store: S,
    ) -> Result<Client<K, S>> {
        debug!("Initializing Noosphere API client");
        debug!("Client represents sphere {}", sphere_identity);
        debug!("Client targetting API at {}", api_base);

        let client = reqwest::Client::new();

        #[cfg(feature = "test-gateway")]
        let forced_host_header = create_test_header(api_base, &Did::from(sphere_identity))?;

        let did_response = {
            let mut url = api_base.clone();
            url.set_path(&v0alpha1::Route::Did.to_string());

            #[allow(unused_mut)]
            let mut client = client.get(url);

            #[cfg(feature = "test-gateway")]
            {
                client = client.header(reqwest::header::HOST, &forced_host_header);
            }

            client.send().await?
        };

        match did_response.status() {
            StatusCode::OK => (),
            _ => return Err(anyhow!("Unable to look up gateway identity")),
        };

        let gateway_identity = did_response.text().await?;

        let mut url = api_base.clone();
        url.set_path(&v0alpha1::Route::Identify.to_string());

        #[allow(unused_mut)]
        let (jwt, mut headers) = Self::make_bearer_token(
            &gateway_identity,
            author,
            &generate_capability(sphere_identity, SphereAbility::Fetch),
            &store,
        )
        .await?;

        #[cfg(feature = "test-gateway")]
        apply_test_header(&mut headers, &forced_host_header);

        let identify_response: v0alpha1::IdentifyResponse = client
            .get(url)
            .bearer_auth(jwt)
            .headers(headers)
            .send()
            .await?
            .json()
            .await?;

        identify_response.verify(did_parser, &store).await?;

        debug!(
            "Handshake succeeded with gateway {}",
            identify_response.gateway_identity
        );

        Ok(Client {
            session: identify_response,
            sphere_identity: sphere_identity.into(),
            api_base: api_base.clone(),
            author: author.clone(),
            store,
            client,
            #[cfg(feature = "test-gateway")]
            forced_host_header,
        })
    }

    async fn make_bearer_token(
        gateway_identity: &str,
        author: &Author<K>,
        capability: &CapabilityView<SphereReference, SphereAbility>,
        store: &S,
    ) -> Result<(String, HeaderMap)> {
        let mut signable = UcanBuilder::default()
            .issued_by(&author.key)
            .for_audience(gateway_identity)
            .with_lifetime(120)
            .claiming_capability(capability)
            .with_nonce()
            .build()?;

        let mut ucan_headers = HeaderMap::new();

        let authorization = author.require_authorization()?;
        let authorization_cid = Cid::try_from(authorization)?;

        match authorization.as_ucan(store).await {
            Ok(ucan) => {
                if let Some(ucan_proofs) = ucan.proofs() {
                    // TODO(ucan-wg/rs-ucan#37): We should integrate a helper for this kind of stuff into rs-ucan
                    let mut proofs_to_search: Vec<String> = ucan_proofs.clone();

                    debug!("Making bearer token... {:?}", proofs_to_search);

                    while let Some(cid_string) = proofs_to_search.pop() {
                        let cid = Cid::from_str(cid_string.as_str())?;
                        let jwt = store.require_token(&cid).await?;
                        let ucan = Ucan::from_str(&jwt)?;

                        debug!("Adding UCAN header for {}", cid);

                        if let Some(ucan_proofs) = ucan.proofs() {
                            proofs_to_search.extend(ucan_proofs.clone().into_iter());
                        }

                        ucan_headers.append("ucan", format!("{cid} {jwt}").parse()?);
                    }
                }

                ucan_headers.append(
                    "ucan",
                    format!("{} {}", authorization_cid, ucan.encode()?).parse()?,
                );
            }
            _ => {
                debug!(
                    "Unable to resolve authorization to a UCAN; it will be used as a blind proof"
                )
            }
        };

        // TODO(ucan-wg/rs-ucan#32): This is kind of a hack until we can add proofs by CID
        signable
            .proofs
            .push(Cid::try_from(authorization)?.to_string());

        let jwt = signable.sign().await?.encode()?;

        trace!("Signing bearer token: {jwt}");

        // TODO: It is inefficient to send the same UCANs with every request,
        // we should probably establish a conventional flow for syncing UCANs
        // this way only once when pairing a gateway. For now, this is about the
        // same efficiency as what we had before when UCANs were all inlined to
        // a single token.
        Ok((jwt, ucan_headers))
    }

    /// Replicate content from Noosphere, streaming its blocks from the
    /// configured gateway.
    ///
    /// If [v0alpha1::ReplicateParameters] are specified, then the replication
    /// will represent incremental history going back to the `since` version.
    ///
    /// Otherwise, the full [crate::data::SphereIpld] will be replicated
    /// (excluding any history).
    ///
    /// If the gateway doesn't have the desired content, it will look it up from
    /// other sources such as IPFS if they are available. Note that this means
    /// this call can potentially block on upstream access to an IPFS node
    /// (which, depending on the node's network configuration and peering
    /// status, can be quite slow).
    pub async fn replicate<R>(
        &self,
        mode: R,
        params: Option<&v0alpha1::ReplicateParameters>,
    ) -> Result<(Cid, impl Stream<Item = Result<(Cid, Vec<u8>)>>)>
    where
        R: Into<ReplicationMode>,
    {
        let mode: ReplicationMode = mode.into();
        let url = Url::try_from(RouteUrl(
            &self.api_base,
            v0alpha1::Route::Replicate(Some(mode.clone())),
            params,
        ))?;

        debug!("Client replicating {} from {}", mode, url);

        let capability = generate_capability(&self.sphere_identity, SphereAbility::Fetch);

        #[allow(unused_mut)]
        let (token, mut headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        #[cfg(feature = "test-gateway")]
        apply_test_header(&mut headers, &self.forced_host_header);

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .headers(headers)
            .send()
            .await?;

        let reader = CarReader::new(StreamReader::new(response.bytes_stream().map(
            |item| match item {
                Ok(item) => Ok(item),
                Err(error) => {
                    error!("Failed to read CAR stream: {}", error);
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                }
            },
        )))
        .await?;

        let root = reader.header().roots().first().cloned().ok_or_else(|| {
            anyhow!(NoosphereError::UnexpectedGatewayResponse(
                "Missing replication root".into()
            ))
        })?;

        Ok((
            root,
            reader.stream().map(|block| match block {
                Ok(block) => Ok(block),
                Err(error) => Err(anyhow!(NoosphereError::UnexpectedGatewayResponse(format!(
                    "Replication stream ended prematurely: {}",
                    error
                )))),
            }),
        ))
    }

    /// Fetch the latest, canonical history of the client's sphere from the
    /// gateway, which serves as the aggregation point for history across many
    /// clients.
    pub async fn fetch(
        &self,
        params: &v0alpha1::FetchParameters,
    ) -> Result<Option<(Link<MemoIpld>, impl Stream<Item = Result<(Cid, Vec<u8>)>>)>> {
        let url = Url::try_from(RouteUrl(
            &self.api_base,
            v0alpha1::Route::Fetch,
            Some(params),
        ))?;

        debug!("Client fetching blocks from {}", url);

        let capability = generate_capability(&self.sphere_identity, SphereAbility::Fetch);
        #[allow(unused_mut)]
        let (token, mut headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        #[cfg(feature = "test-gateway")]
        apply_test_header(&mut headers, &self.forced_host_header);

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .headers(headers)
            .send()
            .await?;

        let reader = CarReader::new(StreamReader::new(response.bytes_stream().map(
            |item| match item {
                Ok(item) => Ok(item),
                Err(error) => {
                    error!("Failed to read CAR stream: {}", error);
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                }
            },
        )))
        .await?;

        let tip = reader.header().roots().first().cloned();

        if let Some(tip) = tip {
            Ok(match tip.codec() {
                // Identity codec = no changes
                0 => None,
                _ => Some((
                    tip.into(),
                    reader.stream().map(|block| match block {
                        Ok(block) => Ok(block),
                        Err(error) => Err(anyhow!(error)),
                    }),
                )),
            })
        } else {
            Ok(None)
        }
    }

    fn make_push_request_stream(
        store: S,
        push_body: v0alpha2::PushBody,
    ) -> impl Stream<Item = Result<Bytes, std::io::Error>> + ConditionalSync + 'static {
        let root = push_body.local_tip.into();
        trace!("Creating push stream...");

        let block_stream = try_stream! {

            let history_stream = memo_history_stream(
                store,
                &push_body.local_tip,
                push_body.local_base.as_ref(),
                true
            );

            trace!("Yielding push body...");
            yield block_serialize::<DagCborCodec, _>(push_body)?;

            for await item in history_stream {
                trace!("Yielding history block...");
                yield item?;
            };
        };

        // Safety: this stream is not shared by us, or by its consumer (reqwest
        // on native targets, gloo-net on web) to others; the [Unshared] is required
        // in order for the wrapped [Stream] to satisfy a `Sync` bound.
        // See: https://github.com/seanmonstar/reqwest/issues/1969
        UnsharedStream::new(Box::pin(to_car_stream(vec![root], block_stream)))
    }

    #[cfg(target_arch = "wasm32")]
    async fn make_push_request(
        &self,
        url: Url,
        headers: HeaderMap,
        token: &str,
        push_body: &v0alpha2::PushBody,
    ) -> Result<impl Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend, v0alpha2::PushError>
    {
        // Implementation note: currently reqwest does not support streaming
        // request bodies under wasm32 targets even though it is technically
        // feasiable via [ReadableStream]. So, we jury rig a one-off streaming
        // request here using wasm-bindgen and wasm-streams:

        use gloo_net::http::Headers;
        use gloo_net::http::Method;
        use gloo_net::http::RequestBuilder;
        use js_sys::{JsString, Uint8Array};
        use wasm_bindgen::JsValue;
        use wasm_streams::ReadableStream;

        let all_headers = Headers::new();
        all_headers.append("Authorization", &format!("Bearer {}", token));

        for (name, value) in headers {
            if let (Some(name), Ok(value)) = (name, value.to_str()) {
                all_headers.append(name.as_str(), value);
            }
        }

        let stream = Self::make_push_request_stream(self.store.clone(), push_body.clone());

        let readable_stream = ReadableStream::from_stream(stream.map(|result| match result {
            Ok(bytes) => Ok(JsValue::from(Uint8Array::from(bytes.as_ref()))),
            Err(error) => Err(JsValue::from(JsString::from(error.to_string()))),
        }));

        let request = RequestBuilder::new(url.as_str())
            .method(Method::PUT)
            .headers(all_headers)
            .body(JsValue::from(readable_stream.as_raw()))
            .map_err(|error| v0alpha2::PushError::Internal(Some(error.to_string())))?;

        let response = request.send().await.map_err(|error| {
            warn!("Push request failed: {}", error);
            v0alpha2::PushError::BrokenUpstream
        })?;

        let body_stream = response
            .body()
            .ok_or_else(|| v0alpha2::PushError::UnexpectedBody)?;
        let body_stream = ReadableStream::from_raw(wasm_bindgen::JsCast::unchecked_into::<
            wasm_streams::readable::sys::ReadableStream,
        >(JsValue::from(body_stream)));

        let car_stream = body_stream.into_stream().map(|result| match result {
            Ok(value) => match Uint8Array::try_from(value) {
                Ok(array) => Ok(Bytes::from(array.to_vec())),
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    v0alpha2::PushError::UnexpectedBody,
                )),
            },
            Err(error) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                error.as_string().unwrap_or_default(),
            )),
        });

        Ok(from_car_stream(car_stream))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn make_push_request(
        &self,
        url: Url,
        headers: HeaderMap,
        token: &str,
        push_body: &v0alpha2::PushBody,
    ) -> Result<impl Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend, v0alpha2::PushError>
    {
        use reqwest::Body;

        let stream = Self::make_push_request_stream(self.store.clone(), push_body.clone());

        let response = self
            .client
            .put(url)
            .bearer_auth(token)
            .headers(headers)
            .header("Content-Type", "application/octet-stream")
            .body(Body::wrap_stream(stream))
            .send()
            .await
            .map_err(|error| {
                warn!("Push request failed: {}", error);
                v0alpha2::PushError::BrokenUpstream
            })?;

        let status = response.status();
        trace!("Checking response ({status})... ");
        if status == StatusCode::CONFLICT {
            return Err(v0alpha2::PushError::Conflict);
        }

        trace!("Fielding response...");

        Ok(from_car_stream(response.bytes_stream()))
    }

    /// Push the latest local history of this client to the gateway
    pub async fn push(
        &self,
        push_body: &v0alpha2::PushBody,
    ) -> Result<v0alpha2::PushResponse, v0alpha2::PushError> {
        let url = Url::try_from(RouteUrl::<_, ()>(
            &self.api_base,
            v0alpha2::Route::Push,
            None,
        ))?;
        debug!(
            "Client pushing changes for sphere {} to {}",
            push_body.sphere, url
        );
        let capability = generate_capability(&self.sphere_identity, SphereAbility::Push);
        #[allow(unused_mut)]
        let (token, mut headers) = Self::make_bearer_token(
            &self.session.gateway_identity,
            &self.author,
            &capability,
            &self.store,
        )
        .await?;

        #[cfg(feature = "test-gateway")]
        apply_test_header(&mut headers, &self.forced_host_header);

        let block_stream = self
            .make_push_request(url, headers, &token, push_body)
            .await?;

        tokio::pin!(block_stream);

        let push_response = match block_stream.try_next().await? {
            Some((_, bytes)) => block_deserialize::<DagCborCodec, v0alpha2::PushResponse>(&bytes)?,
            _ => return Err(v0alpha2::PushError::UnexpectedBody),
        };

        put_block_stream(self.store.clone(), block_stream)
            .await
            .map_err(|error| {
                warn!("Failed to store blocks from gateway: {}", error);
                v0alpha2::PushError::BrokenDownstream
            })?;

        Ok(push_response)
    }
}

#[cfg(feature = "test-gateway")]
fn apply_test_header(headers: &mut HeaderMap, forced_host_header: &reqwest::header::HeaderValue) {
    use reqwest::header::HOST;
    _ = headers.remove(HOST);
    headers.insert(HOST, forced_host_header.to_owned());
}

#[cfg(feature = "test-gateway")]
fn create_test_header(api_base: &Url, identity: &Did) -> Result<reqwest::header::HeaderValue> {
    let mod_identity = identity
        .as_str()
        .strip_prefix("did:key:")
        .ok_or_else(|| anyhow!("Could not format Host header for test-gateway."))?;
    let domain = api_base
        .domain()
        .ok_or_else(|| anyhow!("Host header does not have domain."))?;
    let port = api_base.port();

    let new_host = if let Some(port) = port {
        format!("{}.{}:{}", mod_identity, domain, port)
    } else {
        format!("{}.{}", mod_identity, domain)
    };

    Ok(reqwest::header::HeaderValue::from_str(&new_host)?)
}

#[cfg(all(test, feature = "test-gateway"))]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn it_creates_test_header_from_url() -> Result<()> {
        let identity = Did::from("did:key:z6Mkuj9KHUDzGng3rKPouDgnrJJAk9DiBLRL7nWV4ULMs4E7");
        let mod_id = "z6Mkuj9KHUDzGng3rKPouDgnrJJAk9DiBLRL7nWV4ULMs4E7";
        let expectations = [
            ("http://localhost", format!("{mod_id}.localhost")),
            ("http://localhost:1234", format!("{mod_id}.localhost:1234")),
            ("http://foo.bar", format!("{mod_id}.foo.bar")),
            ("http://foo.bar:1234", format!("{mod_id}.foo.bar:1234")),
        ];

        for (api_base, expected_host) in expectations {
            assert_eq!(
                create_test_header(&Url::parse(api_base)?, &identity)?,
                HeaderValue::from_str(&expected_host)?
            );
        }

        assert!(create_test_header(&Url::parse("http://127.0.0.1")?, &identity).is_err());
        assert!(create_test_header(&Url::parse("http://127.0.0.1:1234")?, &identity).is_err());
        Ok(())
    }
}
