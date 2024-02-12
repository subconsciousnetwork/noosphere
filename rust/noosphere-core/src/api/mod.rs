//! This module contains data structures and client implementation for working
//! with the REST API exposed by Noosphere Gateways.

mod client;
mod data;
mod route;

pub mod headers;
pub mod v0alpha1;
pub mod v0alpha2;

pub use client::*;
pub use data::*;
pub use route::*;
