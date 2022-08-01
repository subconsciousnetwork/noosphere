use anyhow::Result;

pub fn base64_encode(data: &[u8]) -> Result<String> {
    Ok(base64::encode_config(&data, base64::URL_SAFE_NO_PAD))
}

pub fn base64_decode(encoded: &str) -> Result<Vec<u8>> {
    Ok(base64::decode_config(encoded, base64::URL_SAFE_NO_PAD)?)
}
