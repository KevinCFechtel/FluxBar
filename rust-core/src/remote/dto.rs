//! Miniflux wire DTOs.
//!
//! Field names and optionality mirror the Go client models
//! (`miniflux.app/v2/client/model.go`). These types never leak into the
//! domain layer; conversions happen in explicit mapping functions.

use serde::Deserialize;

/// Entry filter mirroring `miniflux.Filter` for the fields FluxBar uses.
///
/// Query-string emission reproduces Go's `buildFilterQueryString` exactly,
/// including alphabetical key ordering and repeated `status` keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntriesFilter {
    pub status: Option<String>,
    pub statuses: Vec<String>,
    pub order: Option<String>,
    pub direction: Option<String>,
    /// Negative disables emission (Go treats `<0` as unset).
    pub limit: i64,
    pub offset: i64,
    pub after_entry_id: i64,
    /// `"1"` restricts to starred entries (Go `FilterOnlyStarred`).
    pub starred: Option<String>,
    pub category_id: i64,
    pub feed_id: i64,
}

impl EntriesFilter {
    /// Emits the query string exactly like Go's builder (sorted keys).
    pub fn to_query(&self) -> String {
        let mut pairs: Vec<(String, String)> = Vec::new();
        if let Some(status) = &self.status {
            pairs.push(("status".into(), status.clone()));
        }
        if let Some(direction) = &self.direction {
            pairs.push(("direction".into(), direction.clone()));
        }
        if let Some(order) = &self.order {
            pairs.push(("order".into(), order.clone()));
        }
        if self.limit >= 0 {
            pairs.push(("limit".into(), self.limit.to_string()));
        }
        if self.offset >= 0 {
            pairs.push(("offset".into(), self.offset.to_string()));
        }
        if self.after_entry_id > 0 {
            pairs.push(("after_entry_id".into(), self.after_entry_id.to_string()));
        }
        if let Some(starred) = &self.starred {
            pairs.push(("starred".into(), starred.clone()));
        }
        if self.category_id > 0 {
            pairs.push(("category_id".into(), self.category_id.to_string()));
        }
        if self.feed_id > 0 {
            pairs.push(("feed_id".into(), self.feed_id.to_string()));
        }
        // Repeated status values keep their slice order among themselves.
        for status in &self.statuses {
            pairs.push(("status".into(), status.clone()));
        }
        pairs.sort_by(|left, right| left.0.cmp(&right.0));
        if pairs.is_empty() {
            return String::new();
        }
        let encoded: Vec<String> = pairs
            .iter()
            .map(|(key, value)| format!("{key}={}", urlencode(value)))
            .collect();
        format!("?{}", encoded.join("&"))
    }
}

/// Percent-encoding matching Go's `url.Values.Encode` (spaces as `%20`,
/// everything outside the unreserved/reserved-safe set escaped).
fn urlencode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'$'
            | b'&'
            | b'+'
            | b','
            | b'/'
            | b':'
            | b';'
            | b'='
            | b'?'
            | b'@' => output.push(byte as char),
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryResultSetDto {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub entries: Vec<EntryDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryDto {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub feed_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub comments_url: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub enclosures: Vec<EnclosureDto>,
    #[serde(default)]
    pub feed: Option<FeedDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnclosureDto {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub mime_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeedDto {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub category: Option<CategoryDto>,
}

impl FeedDto {
    /// Category ID following the Go mapping fallbacks (`mapEntries`).
    pub fn category_id(&self) -> i64 {
        self.category
            .as_ref()
            .map(|category| category.id)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryDto {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeedCountersDto {
    #[serde(default)]
    pub unreads: std::collections::HashMap<String, i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeedIconDto {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(filter: &EntriesFilter) -> String {
        filter.to_query()
    }

    #[test]
    fn query_matches_go_builder_ordering() {
        let mut filter = EntriesFilter {
            limit: -1,
            offset: -1,
            ..Default::default()
        };
        assert_eq!(query(&filter), "");

        filter.limit = 200;
        filter.offset = 0;
        filter.order = Some("id".into());
        filter.direction = Some("asc".into());
        filter.statuses = vec!["read".into(), "unread".into()];
        filter.after_entry_id = 250;
        assert_eq!(
            query(&filter),
            "?after_entry_id=250&direction=asc&limit=200&offset=0&order=id&status=read&status=unread"
        );

        let starred = EntriesFilter {
            starred: Some("1".into()),
            status: Some("unread".into()),
            feed_id: 9,
            category_id: 7,
            limit: 1,
            offset: -1,
            ..Default::default()
        };
        assert_eq!(
            query(&starred),
            "?category_id=7&feed_id=9&limit=1&starred=1&status=unread"
        );
    }

    #[test]
    fn values_are_percent_encoded() {
        let filter = EntriesFilter {
            status: Some("unread".into()),
            order: Some("published_at".into()),
            limit: -1,
            offset: -1,
            ..Default::default()
        };
        assert_eq!(query(&filter), "?order=published_at&status=unread");
    }

    #[test]
    fn decodes_entry_result_set() {
        let payload = r#"{
            "total": 42,
            "entries": [{
                "id": 5, "feed_id": 3, "title": "T", "url": "https://e",
                "comments_url": "https://c", "status": "unread", "starred": true,
                "published_at": "2026-08-22T10:00:00Z",
                "unknown_future_field": {"x": 1},
                "feed": {"id": 3, "title": "F", "category": {"id": 2, "title": "C"}}
            }]
        }"#;
        let result: EntryResultSetDto = serde_json::from_str(payload).unwrap();
        assert_eq!(result.total, 42);
        let entry = &result.entries[0];
        assert_eq!(entry.id, 5);
        assert!(entry.starred);
        assert_eq!(entry.feed.as_ref().unwrap().category_id(), 2);
    }
}
