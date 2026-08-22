//! FluxBar Rust core.
//!
//! This crate exports the same C ABI as the existing Go core:
//!
//! ```c
//! extern char* FluxCoreRequest(char* request);
//! extern void FluxCoreFree(char* value);
//! ```
//!
//! Phase 3 architecture:
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
//! operation-specific handler
//!   │
//!   ▼
//! not implemented
//! ```
//!
//! SQLite, Miniflux, snapshots, icons, localization, mutations, and sync are
//! intentionally not implemented yet. All `unsafe` code is confined to the
//! `ffi` module.

pub mod article;
mod dispatcher;
pub mod domain;
mod ffi;
pub mod persistence;
pub mod remote;
pub mod runtime;
pub mod snapshot;
pub mod sync;
pub mod transport;

// Re-export the C ABI so the crate's `.staticlib` contains the expected
// symbols. The transport and dispatcher modules are crate-private for now.
pub use ffi::{FluxCoreFree, FluxCoreRequest};
