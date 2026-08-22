//! Entry (article) domain model.
//!
//! Ported from `go-core/internal/model/entry.go`. Fields that exist purely
//! for snapshot/UI transport (`icon`, `darkIcon`) are intentionally absent
//! here; they belong to the later snapshot layer.

/// Read state using the exact wire strings from the Go core
/// (`"read"` / `"unread"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStatus {
    Read,
    Unread,
}

impl EntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryStatus::Read => "read",
            EntryStatus::Unread => "unread",
        }
    }

    /// Parses a status string. Returns `None` for unknown values so callers
    /// cannot silently invent states; the Go core never validates this field,
    /// which is tracked as a compatibility observation.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(EntryStatus::Read),
            "unread" => Some(EntryStatus::Unread),
            _ => None,
        }
    }
}

/// A feed article as FluxBar's domain understands it.
///
/// Timestamps use RFC 3339 strings at the domain boundary for now; typed
/// date-time handling arrives with the adapters that produce/consume them
/// (Phase 5+). This keeps the domain layer dependency-free while identity,
/// association, and state semantics are already explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub comments_url: String,
    pub feed_id: i64,
    pub feed_name: String,
    pub category_id: i64,
    pub published_at_rfc3339: String,
    pub preview: String,
    pub image_url: String,
    pub status: EntryStatus,
    pub starred: bool,
}

impl Entry {
    /// Whether this entry belongs to the given selection scope.
    ///
    /// Mirrors the filtering half of Go's SQL `selectionClause`:
    /// - all/unread/starred match every entry of the account;
    /// - category/feed match by their identifier.
    pub fn matches_scope(&self, selection: &super::selection::Selection) -> bool {
        match selection {
            super::selection::Selection::Category { id, .. } => self.category_id == *id,
            super::selection::Selection::Feed { id, .. } => self.feed_id == *id,
            _ => true,
        }
    }

    /// Whether the entry satisfies the read-state restriction of a selection.
    pub fn matches_read_state(&self, selection: &super::selection::Selection) -> bool {
        if !selection.is_unread_only() {
            return true;
        }
        self.status == EntryStatus::Unread
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::selection::Selection;

    fn entry(id: i64, feed_id: i64, category_id: i64, status: EntryStatus) -> Entry {
        Entry {
            id,
            title: format!("Entry {id}"),
            url: format!("https://example.com/{id}"),
            comments_url: String::new(),
            feed_id,
            feed_name: format!("Feed {feed_id}"),
            category_id,
            published_at_rfc3339: "2026-01-02T03:04:05Z".to_string(),
            preview: "text".to_string(),
            image_url: String::new(),
            status,
            starred: false,
        }
    }

    #[test]
    fn status_roundtrip() {
        assert_eq!(EntryStatus::parse("read"), Some(EntryStatus::Read));
        assert_eq!(EntryStatus::parse("unread"), Some(EntryStatus::Unread));
        assert_eq!(EntryStatus::Read.as_str(), "read");
        assert_eq!(EntryStatus::Unread.as_str(), "unread");
        assert_eq!(EntryStatus::parse("bogus"), None);
    }

    #[test]
    fn scope_matching() {
        let unread = entry(1, 10, 100, EntryStatus::Unread);
        let read = entry(2, 11, 100, EntryStatus::Read);

        assert!(unread.matches_scope(&Selection::normalize("category", 100, false)));
        assert!(!unread.matches_scope(&Selection::normalize("feed", 99, false)));
        assert!(read.matches_scope(&Selection::normalize("all", 0, false)));

        assert!(!read.matches_read_state(&Selection::normalize("all", 0, true)));
        assert!(unread.matches_read_state(&Selection::normalize("all", 0, true)));
        // Starred without unreadOnly includes read entries.
        assert!(read.matches_read_state(&Selection::normalize("starred", 0, false)));
        // kind=unread is unread-only even with flag false.
        assert!(!read.matches_read_state(&Selection::normalize("unread", 0, false)));
        assert!(unread.matches_read_state(&Selection::normalize("unread", 0, false)));
    }
}
