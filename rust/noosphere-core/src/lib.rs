#![warn(missing_docs)]

//! This crate embodies the core implementation of the Noosphere protocol.
//!
//! It includes facilities to:
//! - Get low-level access to Noosphere data structures ([view] and [data])
//! - Interact with a Noosphere Gateway ([api::Client])
//! - Read, update, and sync spheres via a high-level API ([context])
//! - And more!

#[macro_use]
extern crate tracing as extern_tracing;

pub mod api;
pub mod authority;
pub mod context;
pub mod data;
pub mod stream;
pub mod view;

pub mod error;
pub mod tracing;

#[cfg(any(test, feature = "helpers"))]
pub mod helpers;
