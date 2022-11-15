use std::{fmt::Display, ops::Deref};

use serde::{Deserialize, Serialize};

/// A helper to stamp out trait implementations that promote coherence between
/// Rust strings and a given wrapper type
macro_rules! string_coherent {
    ($wrapper:ty) => {
        impl Deref for $wrapper {
            type Target = String;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<&str> for $wrapper {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $wrapper {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<$wrapper> for String {
            fn from(value: $wrapper) -> Self {
                value.0
            }
        }

        impl PartialEq<String> for $wrapper {
            fn eq(&self, other: &String) -> bool {
                &self.0 == other
            }
        }

        impl PartialEq<$wrapper> for String {
            fn eq(&self, other: &$wrapper) -> bool {
                self == &other.0
            }
        }

        impl Display for $wrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&self.0, f)
            }
        }

        impl AsRef<[u8]> for $wrapper {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
    };
}

/// A DID, aka a Decentralized Identifier, is a string that can be parsed and
/// resolved into a so-called DID Document, usually in order to obtain PKI
/// details related to a particular user or process.
///
/// See: https://en.wikipedia.org/wiki/Decentralized_identifier
/// See: https://www.w3.org/TR/did-core/
#[repr(transparent)]
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Did(pub String);

string_coherent!(Did);

/// A JWT, aka a JSON Web Token, is a specialized string-encoding of a
/// particular format of JSON and an associated signature, commonly used for
/// authorization flows on the web, but notably also used by the UCAN spec.
///
/// See: https://jwt.io/
/// See: https://ucan.xyz/
#[repr(transparent)]
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Jwt(pub String);

string_coherent!(Jwt);

/// A BIP39-compatible mnemonic phrase that represents the data needed to
/// recover the private half of a cryptographic key pair.
///
/// See: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki
#[repr(transparent)]
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Mnemonic(pub String);

#[cfg(test)]
mod tests {
    use libipld_cbor::DagCborCodec;
    use noosphere_storage::encoding::{block_deserialize, block_serialize};
    use serde::{Deserialize, Serialize};

    use crate::data::Did;

    #[test]
    fn it_serializes_a_did_transparently_as_a_string() {
        #[derive(Serialize, Deserialize)]
        struct FooDid {
            foo: Did,
        }

        #[derive(Serialize, Deserialize)]
        struct FooString {
            foo: String,
        }

        let string_value = String::from("foobar");
        let (did_cid, did_block) = block_serialize::<DagCborCodec, _>(&FooDid {
            foo: Did(string_value.clone()),
        })
        .unwrap();

        let (string_cid, string_block) = block_serialize::<DagCborCodec, _>(&FooString {
            foo: string_value.clone(),
        })
        .unwrap();

        assert_eq!(did_cid, string_cid);
        assert_eq!(did_block, string_block);

        let did_from_string = block_deserialize::<DagCborCodec, FooDid>(&string_block).unwrap();
        let string_from_did = block_deserialize::<DagCborCodec, FooString>(&did_block).unwrap();

        assert_eq!(did_from_string.foo, Did(string_value.clone()));
        assert_eq!(string_from_did.foo, string_value);
    }
}
