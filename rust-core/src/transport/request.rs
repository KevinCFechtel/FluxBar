//! External JSON request envelope and operation-specific typed requests.

use serde::Deserialize;

/// Article selection descriptor shared by many operations.
///
/// This is the wire-level DTO; the domain representation lives in
/// `crate::domain::selection` and is produced via [`Selection::to_domain`].
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Selection {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub id: i64,
    #[serde(default, rename = "unreadOnly")]
    pub unread_only: bool,
}

impl Selection {
    /// Converts the wire DTO into normalized domain semantics.
    pub fn to_domain(&self) -> crate::domain::selection::Selection {
        crate::domain::selection::Selection::normalize(&self.kind, self.id, self.unread_only)
    }
}

/// External request envelope. Fields use the same JSON names as the Go core.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub operation: String,
    #[serde(default)]
    pub server: String,
    #[serde(default, rename = "apiKey")]
    pub api_key: String,
    #[serde(default, rename = "newestFirst")]
    pub newest_first: bool,
    #[serde(default, rename = "configurationGeneration")]
    pub configuration_generation: i64,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub fallback: String,
    #[serde(default, rename = "oneFallback")]
    pub one_fallback: String,
    #[serde(default, rename = "otherFallback")]
    pub other_fallback: String,
    #[serde(default)]
    pub count: i32,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default, rename = "entryID")]
    pub entry_id: i64,
    #[serde(default, rename = "entryIDs")]
    pub entry_ids: Vec<i64>,
    #[serde(default, rename = "retainEntryIDs")]
    pub retain_entry_ids: Vec<i64>,
    #[serde(default)]
    pub read: bool,
    #[serde(default, rename = "mutationSource")]
    pub mutation_source: String,
    #[serde(default, rename = "mutationID")]
    pub mutation_id: String,
    /// Compatibility field ignored by the Go core and not used in Phase 3.
    #[serde(default, rename = "currentStarred")]
    #[allow(dead_code)]
    pub current_starred: bool,
    #[serde(default, rename = "desiredStarred")]
    pub desired_starred: bool,
    #[serde(default, rename = "feedID")]
    pub feed_id: i64,
    #[serde(default, rename = "feedName")]
    pub feed_name: String,
}

impl Request {
    /// Converts the flat external envelope into a typed operation.
    pub fn into_operation(self) -> Result<Operation, String> {
        match self.operation.as_str() {
            "configure" => Ok(Operation::Configure {
                server: self.server,
                api_key: self.api_key,
                newest_first: self.newest_first,
                configuration_generation: self.configuration_generation,
                locales: self.locales,
            }),
            "local_snapshot" => Ok(Operation::LocalSnapshot {
                selection: self.selection,
                retain_entry_ids: self.retain_entry_ids,
            }),
            "refresh" => Ok(Operation::Refresh {
                selection: self.selection,
                retain_entry_ids: self.retain_entry_ids,
            }),
            "set_read" => Ok(Operation::SetRead {
                selection: self.selection,
                entry_id: self.entry_id,
                entry_ids: self.entry_ids,
                retain_entry_ids: self.retain_entry_ids,
                read: self.read,
                mutation_source: self.mutation_source,
            }),
            "set_starred" => Ok(Operation::SetStarred {
                selection: self.selection,
                entry_id: self.entry_id,
                retain_entry_ids: self.retain_entry_ids,
                desired_starred: self.desired_starred,
            }),
            "undo_read" => Ok(Operation::UndoRead {
                selection: self.selection,
                mutation_id: self.mutation_id,
                retain_entry_ids: self.retain_entry_ids,
            }),
            "discard_undo" => Ok(Operation::DiscardUndo {
                mutation_id: self.mutation_id,
            }),
            "flush_pending" => Ok(Operation::FlushPending {
                selection: self.selection,
                retain_entry_ids: self.retain_entry_ids,
            }),
            "feed_icon" => Ok(Operation::FeedIcon {
                feed_id: self.feed_id,
                feed_name: self.feed_name,
            }),
            "localize" => Ok(Operation::Localize {
                locales: self.locales,
                key: self.key,
                fallback: self.fallback,
            }),
            "localize_plural" => Ok(Operation::LocalizePlural {
                locales: self.locales,
                key: self.key,
                one_fallback: self.one_fallback,
                other_fallback: self.other_fallback,
                count: self.count,
            }),
            "" => Err(r#"unsupported operation """#.to_string()),
            other => Err(format!(r#"unsupported operation "{other}""#)),
        }
    }
}

/// Typed internal representation of every supported core operation.
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Configure {
        server: String,
        api_key: String,
        newest_first: bool,
        configuration_generation: i64,
        locales: Vec<String>,
    },
    LocalSnapshot {
        selection: Selection,
        retain_entry_ids: Vec<i64>,
    },
    Refresh {
        selection: Selection,
        retain_entry_ids: Vec<i64>,
    },
    SetRead {
        selection: Selection,
        entry_id: i64,
        entry_ids: Vec<i64>,
        retain_entry_ids: Vec<i64>,
        read: bool,
        mutation_source: String,
    },
    SetStarred {
        selection: Selection,
        entry_id: i64,
        retain_entry_ids: Vec<i64>,
        desired_starred: bool,
    },
    UndoRead {
        selection: Selection,
        mutation_id: String,
        retain_entry_ids: Vec<i64>,
    },
    DiscardUndo {
        mutation_id: String,
    },
    FlushPending {
        selection: Selection,
        retain_entry_ids: Vec<i64>,
    },
    FeedIcon {
        feed_id: i64,
        feed_name: String,
    },
    Localize {
        locales: Vec<String>,
        key: String,
        fallback: String,
    },
    LocalizePlural {
        locales: Vec<String>,
        key: String,
        one_fallback: String,
        other_fallback: String,
        count: i32,
    },
}

#[cfg(test)]
impl Operation {
    /// Returns the external operation string for diagnostics and tests.
    pub fn operation_name(&self) -> &'static str {
        match self {
            Operation::Configure { .. } => "configure",
            Operation::LocalSnapshot { .. } => "local_snapshot",
            Operation::Refresh { .. } => "refresh",
            Operation::SetRead { .. } => "set_read",
            Operation::SetStarred { .. } => "set_starred",
            Operation::UndoRead { .. } => "undo_read",
            Operation::DiscardUndo { .. } => "discard_undo",
            Operation::FlushPending { .. } => "flush_pending",
            Operation::FeedIcon { .. } => "feed_icon",
            Operation::Localize { .. } => "localize",
            Operation::LocalizePlural { .. } => "localize_plural",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_configure_request() {
        let req: Request = serde_json::from_str(
            r#"{"operation":"configure","server":"https://m.example","apiKey":"secret","newestFirst":true,"configurationGeneration":3,"locales":["de-DE"]}"#,
        )
        .unwrap();
        assert_eq!(req.operation, "configure");
        assert_eq!(req.server, "https://m.example");
        assert_eq!(req.api_key, "secret");
        assert!(req.newest_first);
        assert_eq!(req.configuration_generation, 3);
        assert_eq!(req.locales, vec!["de-DE"]);
    }

    #[test]
    fn parses_selection_with_defaults() {
        let req: Request =
            serde_json::from_str(r#"{"operation":"local_snapshot","selection":{"kind":"all"}}"#)
                .unwrap();
        assert_eq!(req.selection.kind, "all");
        assert_eq!(req.selection.id, 0);
        assert!(!req.selection.unread_only);
    }

    #[test]
    fn parses_set_read_payload() {
        let req: Request = serde_json::from_str(
            r#"{"operation":"set_read","selection":{"kind":"feed","id":7,"unreadOnly":true},"entryID":42,"entryIDs":[1,2],"retainEntryIDs":[3],"read":true,"mutationSource":"automatic"}"#,
        )
        .unwrap();
        assert_eq!(req.selection.kind, "feed");
        assert_eq!(req.selection.id, 7);
        assert!(req.selection.unread_only);
        assert_eq!(req.entry_id, 42);
        assert_eq!(req.entry_ids, vec![1, 2]);
        assert_eq!(req.retain_entry_ids, vec![3]);
        assert!(req.read);
        assert_eq!(req.mutation_source, "automatic");
    }

    #[test]
    fn into_operation_routes_every_supported_operation() {
        let operations = vec![
            "configure",
            "local_snapshot",
            "refresh",
            "set_read",
            "set_starred",
            "undo_read",
            "discard_undo",
            "flush_pending",
            "feed_icon",
            "localize",
            "localize_plural",
        ];
        for op in operations {
            let req = Request {
                operation: op.to_string(),
                ..Default::default()
            };
            let result = req.into_operation();
            assert!(result.is_ok(), "{op} should parse");
            assert_eq!(result.unwrap().operation_name(), op);
        }
    }

    #[test]
    fn unknown_operation_returns_error() {
        let req = Request {
            operation: "nope".to_string(),
            ..Default::default()
        };
        let err = req.into_operation().unwrap_err();
        assert_eq!(err, r#"unsupported operation "nope""#);
    }

    #[test]
    fn empty_operation_returns_error() {
        let req = Request {
            operation: "".to_string(),
            ..Default::default()
        };
        let err = req.into_operation().unwrap_err();
        assert_eq!(err, r#"unsupported operation """#);
    }

    #[test]
    fn missing_operation_deserializes_to_empty_and_returns_error() {
        let req: Request = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(req.operation, "");
        let err = req.into_operation().unwrap_err();
        assert_eq!(err, r#"unsupported operation """#);
    }

    #[test]
    fn wire_selection_converts_to_domain() {
        let wire: Selection =
            serde_json::from_str(r#"{"kind":"feed","id":3,"unreadOnly":true}"#).unwrap();
        assert_eq!(
            wire.to_domain(),
            crate::domain::selection::Selection::Feed {
                id: 3,
                unread_only: true
            }
        );

        // Empty/invalid kinds normalize to all+unread-only (Go behavior).
        let empty: Selection = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(
            empty.to_domain(),
            crate::domain::selection::Selection::All {
                id: 0,
                unread_only: true
            }
        );
    }
}
