//! :warning: `noosphere-car` is deprecated.
//! Please use [iroh-car](https://docs.rs/iroh-car).
//!
//! Implementation of the [car](https://ipld.io/specs/transport/car/) format.

#![allow(deprecated)]

mod error;
mod header;
mod reader;
mod util;
mod varint;
mod writer;

#[deprecated(note = "`noosphere-car` is deprecated. Please use `iroh-car`.")]
pub use crate::header::CarHeader;
#[deprecated(note = "`noosphere-car` is deprecated. Please use `iroh-car`.")]
pub use crate::reader::CarReader;
#[deprecated(note = "`noosphere-car` is deprecated. Please use `iroh-car`.")]
pub use crate::writer::CarWriter;
