//! Pure FluxBar domain models.
//!
//! This layer holds platform-independent concepts shared by every future
//! adapter (transport, persistence, remote). It must stay free of serde JSON
//! concerns, C/FFI types, SQLite representations, and Miniflux DTOs.

pub mod account;
pub mod entry;
pub mod navigation;
pub mod selection;
