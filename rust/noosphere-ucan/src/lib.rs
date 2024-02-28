//! Implement UCAN-based authorization with conciseness and ease!
//!
//! [UCANs][UCAN docs] are an emerging pattern based on
//! [JSON Web Tokens][JWT docs] (aka JWTs) that facilitate distributed and/or
//! decentralized authorization flows in web applications. Visit
//! [https://ucan.xyz][UCAN docs] for an introduction to UCANs and ideas for
//! how you can use them in your application.
//!
//! # Examples
//!
//! This crate offers the [`builder::UcanBuilder`] abstraction to generate
//! signed UCAN tokens.
//!
//! To generate a signed token, you need to provide a  [`crypto::KeyMaterial`]
//! implementation. For more information on providing a signing key, see the
//! [`crypto`] module documentation.
//!
//! ```rust
//! use noosphere_ucan::{
//!   builder::UcanBuilder,
//!   crypto::KeyMaterial,
//! };
//!
//! async fn generate_token<'a, K: KeyMaterial>(issuer_key: &'a K, audience_did: &'a str) -> Result<String, anyhow::Error> {
//!     UcanBuilder::default()
//!       .issued_by(issuer_key)
//!       .for_audience(audience_did)
//!       .with_lifetime(60)
//!       .build()?
//!       .sign().await?
//!       .encode()
//! }
//! ```
//!
//! The crate also offers a validating parser to interpret UCAN tokens and
//! the capabilities they grant via their issuer and/or witnessing proofs:
//! the [`chain::ProofChain`].
//!
//! Most capabilities are closely tied to a specific application domain. See the
//! [`capability`] module documentation to read more about defining your own
//! domain-specific semantics.
//!
//! ```rust
//! use noosphere_ucan::{
//!   chain::{ProofChain, CapabilityInfo},
//!   capability::{CapabilitySemantics, Scope, Ability},
//!   crypto::did::{DidParser, KeyConstructorSlice},
//!   store::UcanJwtStore
//! };
//!
//! const SUPPORTED_KEY_TYPES: &KeyConstructorSlice = &[
//!     // You must bring your own key support
//! ];
//!
//! async fn get_capabilities<'a, Semantics, S, A, Store>(ucan_token: &'a str, semantics: &'a Semantics, store: &'a Store) -> Result<Vec<CapabilityInfo<S, A>>, anyhow::Error>
//!     where
//!         Semantics: CapabilitySemantics<S, A>,
//!         S: Scope,
//!         A: Ability,
//!         Store: UcanJwtStore
//! {
//!     let mut did_parser = DidParser::new(SUPPORTED_KEY_TYPES);
//!
//!     Ok(ProofChain::try_from_token_string(ucan_token, None, &mut did_parser, store).await?
//!         .reduce_capabilities(semantics))
//! }
//! ```
//!
//! Note that you must bring your own key support in order to build a
//! `ProofChain`, via a [`crypto::did::DidParser`]. This is so that the core
//! library can remain agnostic of backing implementations for specific key
//! types.
//!
//! [JWT docs]: https://jwt.io/
//! [UCAN docs]: https://ucan.xyz/
//! [DID spec]: https://www.w3.org/TR/did-core/
//! [DID Key spec]: https://w3c-ccg.github.io/did-method-key/

pub mod builder;
pub mod capability;
pub mod chain;
pub mod crypto;
pub mod ipld;
pub mod key_material;
pub mod serde;
pub mod store;
pub mod time;
pub mod ucan;
pub use self::ucan::Ucan;

#[cfg(test)]
mod tests;
