//! Remote Miniflux adapter.
//!
//! HTTP concerns live here only: transport DTOs (`dto`), typed errors
//! (`error`), the blocking client, and Go-compatible pagination. Results are
//! converted into domain-level inputs at explicit boundaries; this layer
//! never touches SQLite, and later sync orchestration (Phase 8) can depend on
//! the [`RemoteInbox`] trait instead of concrete HTTP calls.

pub mod dto;
pub mod error;
pub mod miniflux;

#[cfg(test)]
pub(crate) mod testserver;

#[cfg(test)]
mod miniflux_tests;

pub use dto::{CategoryDto, EntriesFilter, EntryDto, FeedCountersDto, FeedDto, FeedIconDto};
pub use error::RemoteError;
pub use miniflux::MinifluxClient;

use crate::article::{EnclosureInput, PREVIEW_LIMIT, extract, first_image_enclosure_url};
use crate::domain::entry::{Entry as DomainEntry, EntryStatus};
use crate::domain::navigation::{CategoryInput, FeedInput};

/// Converts a remote entry into its domain representation.
///
/// Preserves the Go `mapEntries` quirks: an entry-level feed name comes from
/// the nested feed title, a zero entry feed ID falls back to the feed's own
/// ID, the category ID derives from the nested feed category, and preview/
/// image extraction (including the image-enclosure fallback) mirrors
/// `article.Extract`.
pub fn entry_to_domain(entry: &EntryDto) -> DomainEntry {
    let (feed_id, feed_name, category_id) = match &entry.feed {
        Some(feed) => {
            let id = if entry.feed_id == 0 {
                feed.id
            } else {
                entry.feed_id
            };
            (id, feed.title.clone(), feed.category_id())
        }
        None => (entry.feed_id, String::new(), 0),
    };
    let preview = extract(&entry.content, &entry.url, PREVIEW_LIMIT);
    let image_url = if preview.image_url.is_empty() {
        let enclosures: Vec<EnclosureInput> = entry
            .enclosures
            .iter()
            .map(|enclosure| EnclosureInput {
                url: enclosure.url.clone(),
                mime_type: enclosure.mime_type.clone(),
            })
            .collect();
        first_image_enclosure_url(&enclosures, &entry.url)
    } else {
        preview.image_url
    };
    DomainEntry {
        id: entry.id,
        title: entry.title.clone(),
        url: entry.url.clone(),
        comments_url: entry.comments_url.clone(),
        feed_id,
        feed_name,
        category_id,
        published_at_rfc3339: entry.published_at.clone(),
        preview: preview.text,
        image_url,
        status: EntryStatus::parse(&entry.status),
        starred: entry.starred,
    }
}

/// Converts remote category/feed payloads into navigation inputs.
pub fn dto_mapping_inputs(
    categories: &[CategoryDto],
    feeds: &[FeedDto],
) -> (Vec<CategoryInput>, Vec<FeedInput>) {
    (
        categories
            .iter()
            .map(|category| CategoryInput {
                id: category.id,
                title: category.title.clone(),
            })
            .collect(),
        feeds
            .iter()
            .map(|feed| FeedInput {
                id: feed.id,
                title: feed.title.clone(),
                category_id: feed.category_id(),
            })
            .collect(),
    )
}

/// Remote capabilities FluxBar uses, isolated for Phase 8 orchestration.
///
/// Implemented by [`MinifluxClient`]; tests provide fakes.
pub trait RemoteInbox: Send + Sync {
    /// Applies an absolute public-operation deadline to subsequent calls.
    /// Fakes may ignore it; the HTTP adapter caps each request by the
    /// remaining duration. `None` restores the library-level timeout.
    fn set_operation_deadline(&self, _deadline: Option<std::time::Instant>) {}

    /// Fully paginated ascending-ID fetch reproducing
    /// `fetchCompleteSelection`. Returns entries plus the verified total.
    fn fetch_complete_selection(
        &self,
        filter: &EntriesFilter,
    ) -> Result<(Vec<EntryDto>, i64), RemoteError>;

    fn categories(&self) -> Result<Vec<CategoryDto>, RemoteError>;
    fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError>;
    fn unread_counters(&self) -> Result<FeedCountersDto, RemoteError>;
    fn starred_total(&self) -> Result<i64, RemoteError>;

    /// Raw remote icon data URL; processing/caching belongs to `icons`.
    fn icon_data_url(&self, feed_id: i64) -> Result<Option<String>, RemoteError>;

    /// Fetches an icon under a caller-specific deadline. This remains separate
    /// from the serialized refresh/flush deadline so icon work may overlap it.
    fn icon_data_url_with_deadline(
        &self,
        feed_id: i64,
        _deadline: std::time::Instant,
    ) -> Result<Option<String>, RemoteError> {
        self.icon_data_url(feed_id)
    }

    /// Low-level remote mutations; scheduling/orchestration stays in Phase 8.
    fn set_read_batch(&self, entry_ids: &[i64], read: bool) -> Result<(), RemoteError>;
    fn entry_starred(&self, entry_id: i64) -> Result<bool, RemoteError>;
    fn toggle_starred(&self, entry_id: i64) -> Result<(), RemoteError>;
}
