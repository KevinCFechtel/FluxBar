//! External JSON request envelope and operation-specific typed requests.

use std::fmt;

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_null_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<Vec<Option<T>>>::deserialize(deserializer)?
        .unwrap_or_default()
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

const REQUEST_FIELDS: &[&str] = &[
    "operation",
    "server",
    "apiKey",
    "newestFirst",
    "configurationGeneration",
    "locales",
    "key",
    "fallback",
    "oneFallback",
    "otherFallback",
    "count",
    "selection",
    "entryID",
    "entryIDs",
    "retainEntryIDs",
    "read",
    "mutationSource",
    "mutationID",
    "currentStarred",
    "desiredStarred",
    "feedID",
    "feedName",
];

/// Decodes the public envelope with Go encoding/json's case-insensitive
/// struct-field matching.
pub fn parse_request(json: &str) -> Result<Request, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    let value = deserializer.deserialize_map(RequestMapVisitor)?;
    deserializer.end()?;
    serde_json::from_value(value)
}

struct RequestMapVisitor;

impl<'de> Visitor<'de> for RequestMapVisitor {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a request object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut object = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let canonical = REQUEST_FIELDS
                .iter()
                .find(|field| field.eq_ignore_ascii_case(&key))
                .copied()
                .unwrap_or(key.as_str());
            let value = if canonical == "selection" {
                map.next_value::<Option<Selection>>()?.map_or(
                    serde_json::Value::Null,
                    |selection| {
                        serde_json::json!({
                            "kind": selection.kind,
                            "id": selection.id,
                            "unreadOnly": selection.unread_only,
                        })
                    },
                )
            } else {
                map.next_value::<serde_json::Value>()?
            };
            // Insertion occurs in source order, preserving Go's behavior when
            // case-equivalent keys occur more than once.
            object.insert(canonical.to_string(), value);
        }
        Ok(serde_json::Value::Object(object))
    }
}

/// Article selection descriptor shared by many operations.
///
/// This is the wire-level DTO; the domain representation lives in
/// `crate::domain::selection` and is produced via [`Selection::to_domain`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    pub kind: String,
    pub id: i64,
    pub unread_only: bool,
}

impl<'de> Deserialize<'de> for Selection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SelectionVisitor;

        impl<'de> Visitor<'de> for SelectionVisitor {
            type Value = Selection;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a selection object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut selection = Selection::default();
                while let Some(key) = map.next_key::<String>()? {
                    if key.eq_ignore_ascii_case("kind") {
                        selection.kind = map.next_value::<Option<String>>()?.unwrap_or_default();
                    } else if key.eq_ignore_ascii_case("id") {
                        selection.id = map.next_value::<Option<i64>>()?.unwrap_or_default();
                    } else if key.eq_ignore_ascii_case("unreadOnly") {
                        selection.unread_only =
                            map.next_value::<Option<bool>>()?.unwrap_or_default();
                    } else {
                        map.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(selection)
            }
        }

        deserializer.deserialize_map(SelectionVisitor)
    }
}

impl Selection {
    /// Converts the wire DTO into normalized domain semantics.
    #[allow(dead_code)]
    pub fn to_domain(&self) -> crate::domain::selection::Selection {
        crate::domain::selection::Selection::normalize(&self.kind, self.id, self.unread_only)
    }
}

/// External request envelope. Fields use the same JSON names as the Go core.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Request {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub operation: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub server: String,
    #[serde(
        default,
        rename = "apiKey",
        deserialize_with = "deserialize_null_default"
    )]
    pub api_key: String,
    #[serde(
        default,
        rename = "newestFirst",
        deserialize_with = "deserialize_null_default"
    )]
    pub newest_first: bool,
    #[serde(
        default,
        rename = "configurationGeneration",
        deserialize_with = "deserialize_null_default"
    )]
    pub configuration_generation: i64,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    pub locales: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub key: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub fallback: String,
    #[serde(
        default,
        rename = "oneFallback",
        deserialize_with = "deserialize_null_default"
    )]
    pub one_fallback: String,
    #[serde(
        default,
        rename = "otherFallback",
        deserialize_with = "deserialize_null_default"
    )]
    pub other_fallback: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub count: i64,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub selection: Selection,
    #[serde(
        default,
        rename = "entryID",
        deserialize_with = "deserialize_null_default"
    )]
    pub entry_id: i64,
    #[serde(
        default,
        rename = "entryIDs",
        deserialize_with = "deserialize_null_default_vec"
    )]
    pub entry_ids: Vec<i64>,
    #[serde(
        default,
        rename = "retainEntryIDs",
        deserialize_with = "deserialize_null_default_vec"
    )]
    pub retain_entry_ids: Vec<i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub read: bool,
    #[serde(
        default,
        rename = "mutationSource",
        deserialize_with = "deserialize_null_default"
    )]
    pub mutation_source: String,
    #[serde(
        default,
        rename = "mutationID",
        deserialize_with = "deserialize_null_default"
    )]
    pub mutation_id: String,
    /// Compatibility field ignored by the Go core and not used in Phase 3.
    #[serde(
        default,
        rename = "currentStarred",
        deserialize_with = "deserialize_null_default"
    )]
    #[allow(dead_code)]
    pub current_starred: bool,
    #[serde(
        default,
        rename = "desiredStarred",
        deserialize_with = "deserialize_null_default"
    )]
    pub desired_starred: bool,
    #[serde(
        default,
        rename = "feedID",
        deserialize_with = "deserialize_null_default"
    )]
    pub feed_id: i64,
    #[serde(
        default,
        rename = "feedName",
        deserialize_with = "deserialize_null_default"
    )]
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
        count: i64,
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
    fn explicit_null_fields_use_go_zero_values() {
        let req: Request = serde_json::from_str(
            r#"{"operation":"localize","locales":null,"key":null,"fallback":null,"selection":{"kind":null,"id":null,"unreadOnly":null},"entryIDs":null,"count":null}"#,
        )
        .unwrap();
        assert_eq!(req.locales, Vec::<String>::new());
        assert_eq!(req.key, "");
        assert_eq!(req.fallback, "");
        assert_eq!(req.selection, Selection::default());
        assert_eq!(req.entry_ids, Vec::<i64>::new());
        assert_eq!(req.count, 0);
    }

    #[test]
    fn public_parser_matches_go_case_insensitive_fields_and_null_elements() {
        let req = parse_request(
            r#"{"OPERATION":"set_read","ENTRYIDS":[null,2],"SELECTION":{"KIND":"feed","ID":7,"UNREADONLY":true},"LOCALES":[null,"de-DE"]}"#,
        )
        .unwrap();
        assert_eq!(req.operation, "set_read");
        assert_eq!(req.entry_ids, vec![0, 2]);
        assert_eq!(req.locales, vec!["", "de-DE"]);
        assert_eq!(
            req.selection,
            Selection {
                kind: "feed".to_string(),
                id: 7,
                unread_only: true,
            }
        );
    }

    #[test]
    fn public_parser_uses_last_case_equivalent_key() {
        let req = parse_request(
            r#"{"operation":"unknown","OPERATION":"localize","selection":{"kind":"all","KIND":"feed","id":7}}"#,
        )
        .unwrap();
        assert_eq!(req.operation, "localize");
        assert_eq!(req.selection.kind, "feed");
    }

    #[test]
    fn plural_count_accepts_go_64_bit_int_range() {
        let request: Request =
            serde_json::from_str(r#"{"operation":"localize_plural","count":3000000000}"#).unwrap();
        assert_eq!(request.count, 3_000_000_000);
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
