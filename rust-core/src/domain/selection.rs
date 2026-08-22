//! Article selection and its deterministic normalization.
//!
//! Ported from `go-core/internal/model/browse.go` (`Selection.Normalized`).
//! The observable Go behavior is preserved exactly, including quirks:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Every entry. `id` is carried only because Go's normalized selections
    /// echo an incoming non-zero `id` back in browse snapshots; it never
    /// affects which entries match.
    All {
        id: i64,
        unread_only: bool,
    },
    /// Entries with status "unread". `unreadOnly` is carried unchanged
    /// because Go echoes it, although filtering already treats this kind as
    /// unread-only.
    Unread {
        unread_only: bool,
    },
    /// Starred entries across read states.
    Starred {
        unread_only: bool,
    },
    Category {
        id: i64,
        unread_only: bool,
    },
    Feed {
        id: i64,
        unread_only: bool,
    },
}

impl Selection {
    /// Normalizes a raw wire-level selection into domain semantics.
    ///
    /// Mirrors `Selection.Normalized()`:
    /// - `"all"` keeps id and unreadOnly unchanged;
    /// - `"unread"` / `"starred"` drop the id and keep unreadOnly;
    /// - `"category"` / `"feed"` keep everything when `id > 0`;
    /// - anything else (including empty/unknown kinds or non-positive ids)
    ///   falls back to all + unread-only.
    pub fn normalize(kind: &str, id: i64, unread_only: bool) -> Self {
        match kind {
            "all" => Selection::All { id, unread_only },
            "unread" => Selection::Unread { unread_only },
            "starred" => Selection::Starred { unread_only },
            "category" if id > 0 => Selection::Category { id, unread_only },
            "feed" if id > 0 => Selection::Feed { id, unread_only },
            _ => Selection::All {
                id: 0,
                unread_only: true,
            },
        }
    }

    /// Whether the selection restricts results to unread entries.
    ///
    /// Kind `Unread` counts as unread-only regardless of the flag, matching
    /// the Go selection clause (`unreadOnly || kind == "unread"`).
    pub fn is_unread_only(&self) -> bool {
        match self {
            // Go clause: `unreadOnly || kind == "unread"`.
            Selection::Unread { .. } => true,
            Selection::All { unread_only, .. }
            | Selection::Starred { unread_only }
            | Selection::Category { unread_only, .. }
            | Selection::Feed { unread_only, .. } => *unread_only,
        }
    }

    /// The category/feed identifier for scoped selections; otherwise 0.
    pub fn scope_id(&self) -> i64 {
        match self {
            Selection::Category { id, .. } | Selection::Feed { id, .. } => *id,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_keeps_everything() {
        assert_eq!(
            Selection::normalize("all", 5, false),
            Selection::All {
                id: 5,
                unread_only: false
            }
        );
    }

    #[test]
    fn unread_drops_id() {
        assert_eq!(
            Selection::normalize("unread", 5, true),
            Selection::Unread { unread_only: true }
        );
    }

    #[test]
    fn starred_drops_id() {
        assert_eq!(
            Selection::normalize("starred", 9, false),
            Selection::Starred { unread_only: false }
        );
    }

    #[test]
    fn category_keeps_valid_id() {
        assert_eq!(
            Selection::normalize("category", 7, true),
            Selection::Category {
                id: 7,
                unread_only: true
            }
        );
    }

    #[test]
    fn feed_keeps_valid_id() {
        assert_eq!(
            Selection::normalize("feed", 3, false),
            Selection::Feed {
                id: 3,
                unread_only: false
            }
        );
    }

    #[test]
    fn invalid_inputs_fall_back_to_all_unread_only() {
        let expected = Selection::All {
            id: 0,
            unread_only: true,
        };
        assert_eq!(Selection::normalize("", 4, false), expected);
        assert_eq!(Selection::normalize("bogus", 4, false), expected);
        assert_eq!(Selection::normalize("category", 0, true), expected);
        assert_eq!(Selection::normalize("feed", -1, false), expected);
    }

    #[test]
    fn unread_only_semantics() {
        assert!(!Selection::normalize("all", 0, false).is_unread_only());
        assert!(Selection::normalize("all", 0, true).is_unread_only());
        // kind=unread is unread-only even with flag false (Go clause quirk).
        assert!(Selection::normalize("unread", 0, false).is_unread_only());
        assert!(!Selection::normalize("starred", 0, false).is_unread_only());
        assert!(Selection::normalize("feed", 2, true).is_unread_only());
    }

    #[test]
    fn scope_id_semantics() {
        assert_eq!(Selection::normalize("category", 7, true).scope_id(), 7);
        assert_eq!(Selection::normalize("feed", 3, true).scope_id(), 3);
        assert_eq!(
            Selection::normalize("all", 5, false).scope_id(),
            0,
            "All's echoed id must not act as a scope"
        );
    }
}
