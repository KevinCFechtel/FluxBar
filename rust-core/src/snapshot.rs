//! Presentation assembly: persistence results into transport snapshot DTOs.
//!
//! This module owns the conversion boundary so neither the domain nor the
//! persistence layer learns about JSON, and the transport layer stays free of
//! assembly logic.

use crate::domain::entry::Entry;
use crate::domain::navigation::{Category, Feed};
use crate::domain::selection::Selection;
use crate::persistence::SnapshotData;
use crate::transport::response::{BrowseSnapshot, CategoryDto, EntryDto, FeedDto, SelectionDto};

/// Converts an assembled local snapshot into its wire representation.
///
/// Field omission mirrors Go's JSON tags: zero IDs, empty optional strings,
/// and false flags are omitted.
pub fn assemble(data: &SnapshotData) -> BrowseSnapshot {
    BrowseSnapshot {
        version: data.version,
        selection: selection_dto(&data.selection),
        entries: data.entries.iter().map(entry_dto).collect(),
        categories: if data.categories.is_empty() {
            None
        } else {
            Some(data.categories.iter().map(category_dto).collect())
        },
        total: data.total,
        unread_total: data.unread_total,
        starred_total: data.starred_total,
    }
}

fn selection_dto(selection: &Selection) -> SelectionDto {
    let (kind, id, unread_only) = match selection {
        Selection::All { id, unread_only } => ("all", *id, *unread_only),
        Selection::Unread { unread_only } => ("unread", 0, *unread_only),
        Selection::Starred { unread_only } => ("starred", 0, *unread_only),
        Selection::Category { id, unread_only } => ("category", *id, *unread_only),
        Selection::Feed { id, unread_only } => ("feed", *id, *unread_only),
    };
    SelectionDto {
        kind: kind.to_string(),
        id,
        unread_only,
    }
}

fn entry_dto(entry: &Entry) -> EntryDto {
    EntryDto {
        id: entry.id,
        title: entry.title.clone(),
        url: entry.url.clone(),
        comments_url: entry.comments_url.clone(),
        feed_id: entry.feed_id,
        feed_name: entry.feed_name.clone(),
        category_id: entry.category_id,
        published_at: entry.published_at_rfc3339.clone(),
        preview: entry.preview.clone(),
        image_url: entry.image_url.clone(),
        status: entry.status.as_str().to_string(),
        starred: entry.starred,
    }
}

fn category_dto(category: &Category) -> CategoryDto {
    CategoryDto {
        id: category.id,
        title: category.title.clone(),
        unread_count: category.unread_count,
        feeds: category.feeds.iter().map(feed_dto).collect(),
    }
}

fn feed_dto(feed: &Feed) -> FeedDto {
    FeedDto {
        id: feed.id,
        title: feed.title.clone(),
        category_id: feed.category_id,
        unread_count: feed.unread_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_dto_echo_matches_go_omissions() {
        let all_with_id = assemble_selection(Selection::All {
            id: 5,
            unread_only: false,
        });
        assert_eq!(all_with_id.kind, "all");
        assert_eq!(all_with_id.id, 5);
        assert!(!all_with_id.unread_only);

        // Unread/starred drop the ID entirely on the wire.
        assert_eq!(
            assemble_selection(Selection::Unread { unread_only: false }).id,
            0
        );

        let json = serde_json::to_string(&assemble_selection(Selection::All {
            id: 0,
            unread_only: false,
        }))
        .unwrap();
        assert_eq!(json, r#"{"kind":"all"}"#);
    }

    fn assemble_selection(selection: Selection) -> SelectionDto {
        let data = SnapshotData {
            version: 1,
            selection,
            entries: vec![],
            categories: vec![],
            total: 0,
            unread_total: 0,
            starred_total: 0,
        };
        assemble(&data).selection
    }
}
