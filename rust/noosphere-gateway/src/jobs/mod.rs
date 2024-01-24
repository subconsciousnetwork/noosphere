//! Gateways and other services submit [GatewayJob]s to be completed.
//!
//! Gateways interface with [crate::jobs::JobClient]s to send [crate::jobs::GatewayJob]s
//! to be processed via [crate::jobs::GatewayJobProcessor], providing context
//! via [crate::jobs::GatewayJobContext].
//! Worker management and job distribution is handled by [crate::jobs::worker_queue].
//!
//! [crate::single_tenant::SingleTenantJobClient] is an implementation for a
//! single-tenant gateway in processing this work.
//!
//! In addition to jobs scheduled by gateway requests, gateways expect some
//! periodic scheduling of jobs, like attempting to resolve name records or
//! compact sphere history every few minutes or so.

mod client;
mod job;
mod job_context;
mod job_processor;
pub mod processors;
pub mod worker_queue;

pub use client::*;
pub use job::*;
pub use job_context::*;
pub use job_processor::*;
