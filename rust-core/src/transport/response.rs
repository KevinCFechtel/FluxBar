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
    pub snapshot: Option<BrowseSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
}

/// Wire-form article selection echoed in snapshots (normalized values).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SelectionDto {
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub id: i64,
    #[serde(
        default,
        skip_serializing_if = "std::ops::Not::not",
        rename = "unreadOnly"
    )]
    pub unread_only: bool,
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// Snapshot entry DTO matching `model.Entry` JSON in the Go core.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EntryDto {
    pub id: i64,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub comments_url: String,
    #[serde(rename = "feedID")]
    pub feed_id: i64,
    #[serde(rename = "feedName")]
    pub feed_name: String,
    #[serde(rename = "categoryID", skip_serializing_if = "is_zero", default)]
    pub category_id: i64,
    #[serde(rename = "publishedAt")]
    pub published_at: String,
    pub preview: String,
    #[serde(rename = "imageURL", skip_serializing_if = "String::is_empty", default)]
    pub image_url: String,
    pub status: String,
    pub starred: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FeedDto {
    pub id: i64,
    pub title: String,
    #[serde(rename = "categoryID")]
    pub category_id: i64,
    #[serde(rename = "unreadCount")]
    pub unread_count: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CategoryDto {
    pub id: i64,
    pub title: String,
    #[serde(rename = "unreadCount")]
    pub unread_count: i32,
    pub feeds: Vec<FeedDto>,
}

/// Full browse snapshot payload (`model.BrowseSnapshot`, schema version 1).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BrowseSnapshot {
    pub version: i32,
    pub selection: SelectionDto,
    pub entries: Vec<EntryDto>,
    /// Go marshals a missing category list as JSON `null` (nil slice); this
    /// is reproduced by serializing an empty list as `None`.
    pub categories: Option<Vec<CategoryDto>>,
    pub total: i32,
    #[serde(rename = "unreadTotal")]
    pub unread_total: i32,
    #[serde(rename = "starredTotal")]
    pub starred_total: i32,
}

/// Feed icon payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Icon {
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_bytes"
    )]
    pub regular: Vec<u8>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_bytes"
    )]
    pub dark: Vec<u8>,
}

fn serialize_bytes<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use base64::Engine;
    serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Mutation receipt payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Receipt {
    pub id: String,
    pub count: i32,
}

impl Response {
    /// Successful empty response.
    pub fn ok() -> Self {
        Self {
            ok: true,
            ..Default::default()
        }
    }

    /// Error response with the supplied message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: message.into(),
            ..Default::default()
        }
    }

    /// The Go-compatible "not configured" error.
    pub fn not_configured() -> Self {
        Self::error("Miniflux is not configured")
    }

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

#[cfg(test)]
mod tests {
    use super::{Icon, Response};

    #[test]
    fn icon_bytes_use_go_base64_wire_format_and_omit_empty_variants() {
        let response = Response {
            ok: true,
            icon: Some(Icon {
                regular: vec![0, 1, 2, 255],
                dark: Vec::new(),
            }),
            ..Response::default()
        };

        assert_eq!(
            response.to_json(),
            r#"{"ok":true,"icon":{"regular":"AAEC/w=="}}"#
        );
    }
}
