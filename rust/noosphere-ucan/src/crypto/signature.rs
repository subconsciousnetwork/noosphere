use strum_macros::{Display, EnumString};

// See: https://www.rfc-editor.org/rfc/rfc7518
// See: https://www.rfc-editor.org/rfc/rfc8037.html#appendix-A.4
#[derive(Debug, Display, EnumString, Eq, PartialEq)]
pub enum JwtSignatureAlgorithm {
    EdDSA,
    RS256,
    ES256,
    ES384,
    ES512,
}
