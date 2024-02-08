use crate::crypto::JwtSignatureAlgorithm;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// See <https://github.com/ucan-wg/ts-ucan/blob/99c9fc4f89fc917cf08d7fb09685705876b960f4/packages/default-plugins/src/prefixes.ts#L1-L6>
// See <https://github.com/multiformats/unsigned-varint>
const NONSTANDARD_VARSIG_PREFIX: u64 = 0xd000;
const ES256K_VARSIG_PREFIX: u64 = 0xd0e7;
const BLS12381G1_VARSIG_PREFIX: u64 = 0xd0ea;
const BLS12381G2_VARSIG_PREFIX: u64 = 0xd0eb;
const EDDSA_VARSIG_PREFIX: u64 = 0xd0ed;
const ES256_VARSIG_PREFIX: u64 = 0xd01200;
const ES384_VARSIG_PREFIX: u64 = 0xd01201;
const ES512_VARSIG_PREFIX: u64 = 0xd01202;
const RS256_VARSIG_PREFIX: u64 = 0xd01205;
const EIP191_VARSIG_PREFIX: u64 = 0xd191;

/// A helper for transforming signatures used in JWTs to their UCAN-IPLD
/// counterpart representation and vice-versa
/// Note, not all valid JWT signature algorithms are represented by this
/// library, nor are all valid varsig prefixes
/// See <https://github.com/ucan-wg/ucan-ipld#25-signature>
#[derive(Debug, Eq, PartialEq)]
pub enum VarsigPrefix {
    NonStandard,
    ES256K,
    BLS12381G1,
    BLS12381G2,
    EdDSA,
    ES256,
    ES384,
    ES512,
    RS256,
    EIP191,
}

impl TryFrom<JwtSignatureAlgorithm> for VarsigPrefix {
    type Error = anyhow::Error;

    fn try_from(value: JwtSignatureAlgorithm) -> Result<Self, Self::Error> {
        Ok(match value {
            JwtSignatureAlgorithm::EdDSA => VarsigPrefix::EdDSA,
            JwtSignatureAlgorithm::RS256 => VarsigPrefix::RS256,
            JwtSignatureAlgorithm::ES256 => VarsigPrefix::ES256,
            JwtSignatureAlgorithm::ES384 => VarsigPrefix::ES384,
            JwtSignatureAlgorithm::ES512 => VarsigPrefix::ES512,
        })
    }
}

impl TryFrom<VarsigPrefix> for JwtSignatureAlgorithm {
    type Error = anyhow::Error;

    fn try_from(value: VarsigPrefix) -> Result<Self, Self::Error> {
        Ok(match value {
            VarsigPrefix::EdDSA => JwtSignatureAlgorithm::EdDSA,
            VarsigPrefix::RS256 => JwtSignatureAlgorithm::RS256,
            VarsigPrefix::ES256 => JwtSignatureAlgorithm::ES256,
            VarsigPrefix::ES384 => JwtSignatureAlgorithm::ES384,
            VarsigPrefix::ES512 => JwtSignatureAlgorithm::ES512,
            _ => {
                return Err(anyhow!(
                    "JWT signature algorithm name for {:?} is not known",
                    value
                ))
            }
        })
    }
}

impl FromStr for VarsigPrefix {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        VarsigPrefix::try_from(JwtSignatureAlgorithm::from_str(s)?)
    }
}

impl From<VarsigPrefix> for u64 {
    fn from(value: VarsigPrefix) -> Self {
        match value {
            VarsigPrefix::NonStandard { .. } => NONSTANDARD_VARSIG_PREFIX,
            VarsigPrefix::ES256K => ES256K_VARSIG_PREFIX,
            VarsigPrefix::BLS12381G1 => BLS12381G1_VARSIG_PREFIX,
            VarsigPrefix::BLS12381G2 => BLS12381G2_VARSIG_PREFIX,
            VarsigPrefix::EdDSA => EDDSA_VARSIG_PREFIX,
            VarsigPrefix::ES256 => ES256_VARSIG_PREFIX,
            VarsigPrefix::ES384 => ES384_VARSIG_PREFIX,
            VarsigPrefix::ES512 => ES512_VARSIG_PREFIX,
            VarsigPrefix::RS256 => RS256_VARSIG_PREFIX,
            VarsigPrefix::EIP191 => EIP191_VARSIG_PREFIX,
        }
    }
}

impl TryFrom<u64> for VarsigPrefix {
    type Error = anyhow::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(match value {
            EDDSA_VARSIG_PREFIX => VarsigPrefix::EdDSA,
            RS256_VARSIG_PREFIX => VarsigPrefix::RS256,
            ES256K_VARSIG_PREFIX => VarsigPrefix::ES256K,
            BLS12381G1_VARSIG_PREFIX => VarsigPrefix::BLS12381G1,
            BLS12381G2_VARSIG_PREFIX => VarsigPrefix::BLS12381G2,
            EIP191_VARSIG_PREFIX => VarsigPrefix::EIP191,
            ES256_VARSIG_PREFIX => VarsigPrefix::ES256,
            ES384_VARSIG_PREFIX => VarsigPrefix::ES384,
            ES512_VARSIG_PREFIX => VarsigPrefix::ES512,
            NONSTANDARD_VARSIG_PREFIX => VarsigPrefix::NonStandard,
            _ => return Err(anyhow!("Signature does not have a recognized prefix")),
        })
    }
}

/// An envelope for the UCAN-IPLD-equivalent of a UCAN's JWT signature, which
/// is a specified prefix in front of the raw signature bytes
/// See: <https://github.com/ucan-wg/ucan-ipld#25-signature>
#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(pub Vec<u8>);

impl Signature {
    pub fn decode(&self) -> Result<(JwtSignatureAlgorithm, Vec<u8>)> {
        let buffer = self.0.as_slice();
        let (prefix, buffer) =
            unsigned_varint::decode::u64(buffer).map_err(|e| anyhow!("{}", e))?;
        let (signature_length, buffer) =
            unsigned_varint::decode::usize(buffer).map_err(|e| anyhow!("{}", e))?;

        // TODO: Non-standard algorithm support here...

        let algorithm = JwtSignatureAlgorithm::try_from(VarsigPrefix::try_from(prefix)?)?;
        let signature = buffer[..signature_length].to_vec();

        Ok((algorithm, signature))
    }
}

// TODO: Support non-standard signature algorithms for experimental purposes
// Note that non-standard signatures should additionally have the signature name
// appended after the signature bytes in the varsig representation
impl<T: AsRef<[u8]>> TryFrom<(JwtSignatureAlgorithm, T)> for Signature {
    type Error = anyhow::Error;

    fn try_from((algorithm, signature): (JwtSignatureAlgorithm, T)) -> Result<Self, Self::Error> {
        // TODO: Non-standard JWT algorithm support here
        let signature_bytes = signature.as_ref();
        let prefix = VarsigPrefix::try_from(algorithm)?;
        let mut prefix_buffer = unsigned_varint::encode::u64_buffer();
        let prefix_bytes = unsigned_varint::encode::u64(prefix.into(), &mut prefix_buffer);
        let mut size_buffer = unsigned_varint::encode::usize_buffer();

        let size_bytes = unsigned_varint::encode::usize(signature_bytes.len(), &mut size_buffer);

        Ok(Signature(
            [prefix_bytes, size_bytes, signature_bytes].concat(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{crypto::JwtSignatureAlgorithm, ipld::Signature};

    use base64::Engine;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn it_can_convert_between_jwt_and_bytesprefix_form() {
        let token_signature = "Ab-xfYRoqYEHuo-252MKXDSiOZkLD-h1gHt8gKBP0AVdJZ6Jruv49TLZOvgWy9QkCpiwKUeGVbHodKcVx-azCQ";
        let signature_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(token_signature)
            .unwrap();

        let bytesprefix_signature =
            Signature::try_from((JwtSignatureAlgorithm::EdDSA, &signature_bytes)).unwrap();

        let (decoded_algorithm, decoded_signature_bytes) = bytesprefix_signature.decode().unwrap();

        assert_eq!(decoded_algorithm, JwtSignatureAlgorithm::EdDSA);
        assert_eq!(decoded_signature_bytes, signature_bytes);
    }

    #[allow(dead_code)]
    // #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[ignore = "Support non-standard signature algorithms"]
    fn it_can_convert_between_jwt_and_bytesprefix_for_nonstandard_signatures() {}
}
