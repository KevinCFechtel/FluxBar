//! FluxBar Rust core.
//!
//! This crate exports the same C ABI as the existing Go core:
//!
//! ```c
//! extern char* FluxCoreRequest(char* request);
//! extern void FluxCoreFree(char* value);
//! ```
//!
//! Compatibility architecture:
//!
//! ```text
//! C ABI (ffi)
//!   │
//!   ▼
//! JSON compatibility adapter (transport)
//!   │
//!   ▼
//! typed operation request (transport::Operation)
//!   │
//!   ▼
//! dispatcher
//!   │
//!   ▼
//! operation-specific runtime/service handler
//! ```
//!
//! All current public operations are implemented. Go remains the behavioral
//! reference while Phase 10 orchestration/deadline risks are remediated. All
//! `unsafe` code is confined to the `ffi` module.

pub mod article;
mod dispatcher;
pub mod domain;
mod ffi;
pub mod icons;
pub mod localization;
pub mod persistence;
pub mod remote;
pub mod runtime;
pub mod snapshot;
pub mod sync;
pub mod transport;

// Re-export the C ABI so the crate's `.staticlib` contains the expected
// symbols. The transport and dispatcher modules are crate-private for now.
pub use ffi::{FluxCoreFree, FluxCoreRequest};
