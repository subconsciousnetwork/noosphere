use anyhow::Result;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use libipld_core::{
    codec::{Codec, Decode, Encode},
    ipld::Ipld,
    serde::{from_ipld, to_ipld},
};
use serde::{de::DeserializeOwned, Serialize};

/// Encode some bytes as an unpadded URL-safe base64 string
pub fn base64_encode(data: &[u8]) -> Result<String> {
    Ok(base64::encode_config(&data, base64::URL_SAFE_NO_PAD))
}

/// Decode some bytes from an unpadded URL-safe base64 string
pub fn base64_decode(encoded: &str) -> Result<Vec<u8>> {
    Ok(base64::decode_config(encoded, base64::URL_SAFE_NO_PAD)?)
}

/// Produces a CID for a block with a Blake2b hash; note that the bytes are
/// presumed to be encoded with the specified codec (honor system; this is
/// not validated in any way).
pub fn derive_cid<C>(block: &[u8]) -> Cid
where
    C: Codec + Default,
    u64: From<C>,
{
    Cid::new_v1(u64::from(C::default()), Code::Blake2b256.digest(block))
}

/// Encode any encodable type as a block using the specified codec
pub fn block_encode<C, T>(encodable: &T) -> Result<(Cid, Vec<u8>)>
where
    C: Codec + Default,
    T: Encode<C>,
    u64: From<C>,
{
    let codec = C::default();
    let block = codec.encode(encodable)?;

    Ok((derive_cid(&block), block))
}

/// Decode any block as IPLD using the specified codec
pub fn block_decode<C, Ipld>(block: &[u8]) -> Result<Ipld>
where
    C: Codec + Default,
    Ipld: Decode<C>,
{
    C::default().decode::<Ipld>(block)
}

/// Encode any serializable type as a block using the specified codec
pub fn block_serialize<C, T>(any: T) -> Result<(Cid, Vec<u8>)>
where
    C: Codec + Default,
    T: Serialize,
    Ipld: Encode<C>,
    u64: From<C>,
{
    block_encode(&to_ipld(any)?)
}

/// Decode any block as a deserializable type using the specified codec
pub fn block_deserialize<C, T>(block: &[u8]) -> Result<T>
where
    C: Codec + Default,
    T: DeserializeOwned,
    Ipld: Decode<C>,
{
    Ok(from_ipld(block_decode(block)?)?)
}
