//! External JSON response envelope and placeholder response payload types.
//!
//! The [`Response`] struct mirrors the common envelope produced by the Go
//! core. Fields that depend on domain behavior not yet ported to Rust use
//! narrowly scoped placeholder types so the envelope shape is preserved
//! without implementing the underlying business logic.

use serde::Serialize;

/// Common response envelope shared by every core operation.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub error: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Snapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
}

/// Placeholder browse snapshot payload.
///
/// Phase 3 does not yet produce real snapshots; this type preserves the
/// field name so future phases can expand it without changing the envelope.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Snapshot {
    pub version: i32,
}

/// Feed icon payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Icon {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regular: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dark: Vec<u8>,
}

/// Mutation receipt payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Receipt {
    pub id: String,
    pub count: i32,
}

impl Response {
    /// Builds the standard Phase 3 "not implemented" response for a known
    /// operation. This matches the deterministic skeleton behavior from
    /// Phase 2 while now routing through the typed dispatcher.
    pub fn not_implemented(operation: &str) -> Self {
        Self {
            ok: false,
            error: format!("not implemented: {operation}"),
            ..Default::default()
        }
    }

    /// Builds an invalid-request error response, escaping the reason via
    /// serde_json during final serialization.
    pub fn invalid_request(reason: &str) -> Self {
        Self {
            ok: false,
            error: format!("invalid request: {reason}"),
            ..Default::default()
        }
    }

    /// Builds the null-request error response.
    pub fn null_request() -> Self {
        Self {
            ok: false,
            error: "null request".to_string(),
            ..Default::default()
        }
    }

    /// Builds the unsupported-operation error response.
    #[allow(dead_code)]
    pub fn unsupported_operation(operation: &str) -> Self {
        Self {
            ok: false,
            error: format!(r#"unsupported operation "{operation}""#),
            ..Default::default()
        }
    }

    /// Serializes the response to a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"encode response"}"#.to_string())
    }
}
