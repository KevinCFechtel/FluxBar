//! JSON transport compatibility layer.
//!
//! This module owns the external request/response shapes required by the
//! existing Go core contract. It deliberately separates the wire format from
//! future Rust domain types so that awkward historical JSON fields do not
//! leak into the domain model.

pub mod request;
pub mod response;

pub use request::{Operation, Request};
pub use response::Response;
