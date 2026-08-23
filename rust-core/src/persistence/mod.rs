//! SQLite persistence adapter for the existing FluxBar database format.
//!
//! The adapter receives an explicit path and does not discover platform
//! directories. It preserves the Go schema and connection contract while
//! keeping SQLite representations outside the domain layer.

mod schema;
mod store;

pub use store::{
    MutationReceipt, OpenError, PendingMutation, PersistedEntry, SNAPSHOT_LIMIT, SnapshotData,
    Store, sqlite_error_category, sqlite_error_summary,
};
