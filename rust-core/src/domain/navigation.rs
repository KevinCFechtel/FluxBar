//! Feed and category navigation models.
//!
//! Ported from the pure halves of `go-core/internal/model/browse.go`
//! (`Feed`, `Category`) and the deterministic transformation in
//! `go-core/internal/miniflux/service.go` (`mapNavigation`): building the
//! category/feed tree, aggregating unread counts, and case-insensitive
//! stable sorting by title.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Feed {
    pub id: i64,
    pub title: String,
    pub category_id: i64,
    pub unread_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Category {
    pub id: i64,
    pub title: String,
    pub unread_count: i32,
    pub feeds: Vec<Feed>,
}

/// Input describing one remote category (identifier + title).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryInput {
    pub id: i64,
    pub title: String,
}

/// Input describing one remote feed (identifier, title, owning category).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedInput {
    pub id: i64,
    pub title: String,
    pub category_id: i64,
}

/// Builds sorted navigation from categories, feeds, and per-feed unread
/// counters.
///
/// Preserves `mapNavigation` semantics:
/// - feeds without a known parent category are skipped;
/// - a category's unread count is the sum of its feeds' counters;
/// - the returned unread total is the sum over all feeds;
/// - categories and feeds are sorted case-insensitively by title using a
///   stable sort (ties keep input order).
pub fn build_navigation(
    categories: &[CategoryInput],
    feeds: &[FeedInput],
    unread_counters: &std::collections::HashMap<i64, i32>,
) -> (Vec<Category>, i32) {
    let mut result: Vec<Category> = categories
        .iter()
        .map(|category| Category {
            id: category.id,
            title: category.title.clone(),
            unread_count: 0,
            feeds: Vec::new(),
        })
        .collect();

    let mut indexes = std::collections::HashMap::with_capacity(categories.len());
    for (index, category) in result.iter().enumerate() {
        indexes.insert(category.id, index);
    }

    let mut unread_total = 0;
    for feed in feeds {
        let Some(&index) = indexes.get(&feed.category_id) else {
            continue;
        };
        let unread_count = unread_counters.get(&feed.id).copied().unwrap_or(0);
        result[index].unread_count += unread_count;
        unread_total += unread_count;
        result[index].feeds.push(Feed {
            id: feed.id,
            title: feed.title.clone(),
            category_id: feed.category_id,
            unread_count,
        });
    }

    sort_by_title(&mut result);
    for category in &mut result {
        sort_by_title(&mut category.feeds);
    }

    (result, unread_total)
}

fn sort_by_title<T>(items: &mut [T])
where
    T: AsTitle,
{
    items.sort_by(|a, b| title_key(a).cmp(&title_key(b)));
}

fn title_key<T: AsTitle>(item: &T) -> String {
    item.domain_title().to_lowercase()
}

trait AsTitle {
    fn domain_title(&self) -> &str;
}

impl AsTitle for Category {
    fn domain_title(&self) -> &str {
        &self.title
    }
}

impl AsTitle for Feed {
    fn domain_title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn counters(pairs: &[(i64, i32)]) -> HashMap<i64, i32> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn builds_tree_and_aggregates_counts() {
        let categories = vec![
            CategoryInput {
                id: 1,
                title: "Tech".to_string(),
            },
            CategoryInput {
                id: 2,
                title: "News".to_string(),
            },
        ];
        let feeds = vec![
            FeedInput {
                id: 10,
                title: "Rust Blog".to_string(),
                category_id: 1,
            },
            FeedInput {
                id: 11,
                title: "Daily".to_string(),
                category_id: 2,
            },
            FeedInput {
                id: 12,
                title: "Orphan".to_string(),
                category_id: 99,
            },
        ];
        let (navigation, total) = build_navigation(
            &categories,
            &feeds,
            &counters(&[(10, 3), (11, 4), (12, 100)]),
        );

        assert_eq!(total, 7, "orphan feed counter must not count");
        assert_eq!(navigation.len(), 2);

        // Case-insensitive sort puts News before Tech despite input order.
        assert_eq!(navigation[0].title, "News");
        assert_eq!(navigation[0].unread_count, 4);
        assert_eq!(navigation[1].title, "Tech");
        assert_eq!(navigation[1].unread_count, 3);
        assert_eq!(navigation[1].feeds[0].id, 10);
        assert_eq!(navigation[1].feeds[0].category_id, 1);
    }

    #[test]
    fn sorting_is_case_insensitive_and_stable() {
        let categories = vec![
            CategoryInput {
                id: 1,
                title: "beta".to_string(),
            },
            CategoryInput {
                id: 2,
                title: "Alpha".to_string(),
            },
            CategoryInput {
                id: 3,
                title: "ALPHA".to_string(),
            },
        ];
        let (navigation, _) = build_navigation(&categories, &[], &counters(&[]));

        // "Alpha" and "ALPHA" tie case-insensitively; stable sort keeps the
        // original relative order.
        let titles: Vec<&str> = navigation.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "ALPHA", "beta"]);
    }

    #[test]
    fn empty_inputs_produce_empty_navigation() {
        let (navigation, total) = build_navigation(&[], &[], &counters(&[]));
        assert!(navigation.is_empty());
        assert_eq!(total, 0);
    }
}
