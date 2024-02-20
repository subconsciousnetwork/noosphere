use crate::crypto::did::{DID_KEY_PREFIX, DID_PREFIX};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

// Note: varint encoding of 0x0d1d
pub const DID_IPLD_PREFIX: &[u8] = &[0x9d, 0x1a];

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principle(Vec<u8>);

impl FromStr for Principle {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Some(stripped) = input.strip_prefix(DID_KEY_PREFIX) {
            Ok(Principle(bs58::decode(stripped).into_vec()?))
        } else if let Some(stripped) = input.strip_prefix(DID_PREFIX) {
            Ok(Principle([DID_IPLD_PREFIX, stripped.as_bytes()].concat()))
        } else {
            Err(anyhow!("This is not a DID: {}", input))
        }
    }
}

impl Display for Principle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = &self.0;
        let did_content = match &bytes[0..2] {
            DID_IPLD_PREFIX => [
                DID_PREFIX,
                std::str::from_utf8(&bytes[2..]).map_err(|_| std::fmt::Error)?,
            ]
            .concat(),
            _ => [DID_KEY_PREFIX, &bs58::encode(bytes).into_string()].concat(),
        };

        write!(f, "{did_content}")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{ipld::Principle, tests::helpers::dag_cbor_roundtrip};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn it_round_trips_a_principle_did() {
        let did_string = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK";
        let principle = dag_cbor_roundtrip(&Principle::from_str(&did_string).unwrap()).unwrap();
        assert_eq!(did_string, principle.to_string());

        let did_string = "did:web:example.com";
        let principle = dag_cbor_roundtrip(&Principle::from_str(&did_string).unwrap()).unwrap();
        assert_eq!(did_string, principle.to_string());
    }
}
