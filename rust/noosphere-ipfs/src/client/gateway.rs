use super::{IpfsClient, IpfsClientAsyncReadSendSync};
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use reqwest::Client;
use reqwest::StatusCode;
use std::str::FromStr;
use url::Url;

/// A high-level HTTP client for accessing IPFS
/// [HTTP Gateway](https://docs.ipfs.tech/reference/http/gateway/) and normalizing
/// their expected payloads to Noosphere-friendly formats.
#[derive(Clone)]
pub struct GatewayClient {
    client: Client,
    api_url: Url,
}

impl GatewayClient {
    pub fn new(api_url: Url) -> Self {
        let client = Client::new();
        GatewayClient { client, api_url }
    }

    pub(crate) fn make_block_url(&self, cid: &Cid) -> Url {
        let mut url = self.api_url.clone();

        if let Some(domain) = url.domain() {
            let mut parts = domain.split('.');

            if let Some(fragment) = parts.nth(0) {
                if Cid::from_str(fragment).is_ok() {
                    let upper_domain = parts
                        .map(|part| part.to_string())
                        .collect::<Vec<String>>()
                        .join(".");

                    let mut host = format!("{}.{}", cid, upper_domain);

                    if let Some(port) = url.port() {
                        host = format!("{}:{}", host, port);
                    }

                    if let Ok(()) = url.set_host(Some(&host)) {
                        return url;
                    }
                }
            }
        }

        url.set_path(&format!("/ipfs/{}", cid));
        url
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl IpfsClient for GatewayClient {
    async fn block_is_pinned(&self, _cid: &Cid) -> Result<bool> {
        unimplemented!("IPFS HTTP Gateway does not have this capability.");
    }

    async fn server_identity(&self) -> Result<String> {
        unimplemented!("IPFS HTTP Gateway does not have this capability.");
    }

    async fn syndicate_blocks<R>(&self, _car: R) -> Result<()>
    where
        R: IpfsClientAsyncReadSendSync,
    {
        unimplemented!("IPFS HTTP Gateway does not have this capability.");
    }

    async fn put_block(&mut self, _cid: &Cid, _block: &[u8]) -> Result<()> {
        unimplemented!("IPFS HTTP Gateway does not have this capability.");
    }

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        let api_url = self.make_block_url(cid);
        let response = self
            .client
            .get(api_url)
            .header("Accept", "application/vnd.ipld.raw")
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => Ok(Some(response.bytes().await?.into())),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_can_derive_a_block_url_for_subdomain_gateways() {
        let gateway_url = Url::from_str(
            "https://bafybeieh53mh2gt4khnrixfro7wvbvtrux4247cfwse642e36z67medkzq.ipfs.noo.pub",
        )
        .unwrap();
        let test_cid =
            Cid::from_str("bafy2bzacecsjls67zqx25dcvbu6p4z4rsdkm2k6hanhd5qowrvwmhtov2sjpo")
                .unwrap();
        let client = GatewayClient::new(gateway_url.clone());
        let derived_url = client.make_block_url(&test_cid);
        let expected_url = Url::from_str(
            "https://bafy2bzacecsjls67zqx25dcvbu6p4z4rsdkm2k6hanhd5qowrvwmhtov2sjpo.ipfs.noo.pub",
        )
        .unwrap();

        assert_eq!(derived_url, expected_url);
    }
}
