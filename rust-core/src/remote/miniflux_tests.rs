//! Fake-server tests for the Miniflux adapter.

use super::testserver::FakeServer;
use crate::remote::RemoteInbox;
use crate::remote::dto::{EntriesFilter, FeedCountersDto};
use crate::remote::error::RemoteError;
use crate::remote::miniflux::{
    BROWSE_PAGE_SIZE, FILTER_ONLY_STARRED, MinifluxClient, STATUS_READ, STATUS_UNREAD,
};

const KEY: &str = "fake-secret-key";

fn entry_json(id: i64) -> String {
    format!(
        r#"{{"id":{id},"feed_id":3,"title":"T{id}","url":"https://e/{id}","comments_url":"","status":"unread","starred":false,"published_at":"2026-08-22T10:{:02}:00Z","content":"<p>body</p>","feed":{{"id":3,"title":"F","category":{{"id":2,"title":"C"}}}}}}"#,
        id % 60
    )
}

fn page(entries: &[i64], total: i64) -> (u16, String) {
    let items: Vec<String> = entries.iter().map(|id| entry_json(*id)).collect();
    (
        200,
        format!(r#"{{"total":{total},"entries":[{}]}}"#, items.join(",")),
    )
}

#[test]
fn auth_header_user_agent_and_url_joining() {
    let server = FakeServer::start(vec![page(&[1], 1)]);
    let client = MinifluxClient::new(&format!("{}/", server.base_url), KEY).expect("client");
    let filter = EntriesFilter {
        limit: 200,
        order: Some("id".into()),
        direction: Some("asc".into()),
        statuses: vec![STATUS_READ.into(), STATUS_UNREAD.into()],
        ..Default::default()
    };
    client.entries(&filter).expect("entries");

    let request = server.next_request();
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.path,
        "/v1/entries?direction=asc&limit=200&offset=0&order=id&status=read&status=unread"
    );
    assert_eq!(request.auth_token.as_deref(), Some(KEY));
}

#[test]
fn empty_endpoint_is_rejected_without_network() {
    assert!(matches!(
        MinifluxClient::new("", KEY),
        Err(RemoteError::Transport(_))
    ));
}

#[test]
fn pagination_single_page_and_exact_boundary() {
    // One short page below the size threshold terminates via total match.
    let server = FakeServer::start(vec![page(&[5, 6], 2)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    let (entries, total) = client
        .fetch_complete_selection(&EntriesFilter {
            limit: BROWSE_PAGE_SIZE,
            offset: -1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!((entries.len(), total), (2, 2));

    // Exactly one full page whose count equals total.
    let ids: Vec<i64> = (1..=200).collect();
    let server = FakeServer::start(vec![page(&ids, 200)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    let (entries, total) = client
        .fetch_complete_selection(&EntriesFilter {
            limit: BROWSE_PAGE_SIZE,
            offset: -1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!((entries.len(), total), (200, 200));
}

#[test]
fn pagination_multi_page_uses_after_entry_id_cursor() {
    let first_ids: Vec<i64> = (1..=200).collect();
    let second_ids: Vec<i64> = (201..=250).collect();
    let server = FakeServer::start(vec![page(&first_ids, 250), page(&second_ids, 250)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    let (entries, total) = client
        .fetch_complete_selection(&EntriesFilter {
            limit: BROWSE_PAGE_SIZE,
            offset: -1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!((entries.len(), total), (250, 250));
    assert_eq!(entries[0].id, 1);
    assert_eq!(entries[249].id, 250);

    // First request has no cursor; second must carry
    // after_entry_id=<last ID of the first page>.
    let first_request = server.next_request();
    assert!(!first_request.path.contains("after_entry_id"));
    let second_request = server.next_request();
    assert!(
        second_request.path.contains("after_entry_id=200"),
        "cursor missing: {}",
        second_request.path
    );
}

#[test]
fn pagination_rejects_duplicate_reordered_short_and_growing_results() {
    // Repeated IDs across pages: Go's ascending-stability guard fires before
    // the dedup check on every realizable sequence, so either German
    // pagination error is the correct compatibility outcome.
    let server = FakeServer::start(vec![page(&[1, 2], 3), page(&[2, 3], 3)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    assert!(matches!(
        client.fetch_complete_selection(&EntriesFilter {
            limit: -1,
            offset: -1,
            ..Default::default()
        }),
        Err(RemoteError::Transport(message))
            if message.contains("doppelter Artikel")
                || message.contains("unstabile Seitensortierung")
                || message.contains("unvollständige paginierte Antwort")
    ));

    // Reordered / non-ascending within a page.
    let server = FakeServer::start(vec![page(&[2, 1], 2)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    assert!(matches!(
        client.fetch_complete_selection(&EntriesFilter {
            limit: 200,
            offset: -1,
            ..Default::default()
        }),
        Err(RemoteError::Transport(message))
            if message.contains("unstabile Seitensortierung")
    ));

    // Short page while total unmet.
    let server = FakeServer::start(vec![page(&[1], 5)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    assert!(matches!(
        client.fetch_complete_selection(&EntriesFilter {
            limit: 200,
            offset: -1,
            ..Default::default()
        }),
        Err(RemoteError::Transport(message))
            if message.contains("unvollständige paginierte Antwort")
    ));

    // Total grows during pagination: second full page overshoots the
    // originally reported total.
    let first_ids: Vec<i64> = (1..=200).collect();
    let second_ids: Vec<i64> = (201..=400).collect();
    let server = FakeServer::start(vec![page(&first_ids, 250), page(&second_ids, 202)]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    assert!(matches!(
        client.fetch_complete_selection(&EntriesFilter {
            limit: 200,
            offset: -1,
            ..Default::default()
        }),
        Err(RemoteError::Transport(message)) if message.contains("Trefferzahl")
    ));
}

#[test]
fn http_error_taxonomy_matches_go_client() {
    let cases: Vec<(u16, String, RemoteError)> = vec![
        (401, "{}".into(), RemoteError::NotAuthorized),
        (403, "{}".into(), RemoteError::Forbidden),
        (404, "{}".into(), RemoteError::NotFound),
        (
            400,
            r#"{"error_message":"bad payload"}"#.into(),
            RemoteError::BadRequest(Some("bad payload".into())),
        ),
        (
            500,
            r#"{"error_message":"boom"}"#.into(),
            RemoteError::ServerError(Some("boom".into())),
        ),
        (500, "not-json".into(), RemoteError::ServerError(None)),
        (429, "{}".into(), RemoteError::Status(429)),
    ];
    for (status, body, expected) in cases {
        let server = FakeServer::start(vec![(status, body)]);
        let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
        let error = client
            .entries(&EntriesFilter {
                offset: -1,
                ..Default::default()
            })
            .expect_err("expected error");
        assert_eq!(error, expected, "status {status}");
    }
}

#[test]
fn malformed_success_json_is_a_json_error() {
    let server = FakeServer::start(vec![(200, "{not json".to_string())]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    assert!(matches!(
        client.entries(&EntriesFilter {
            offset: -1,
            ..Default::default()
        }),
        Err(RemoteError::Json(_))
    ));
}

#[test]
fn counters_feeds_icon_and_mutations_wire_format() {
    let counters_body = r#"{"reads":{"3":7},"unreads":{"3":12,"9":0}}"#;
    let feeds_body = r#"[{"id":3,"title":"F","category":{"id":2,"title":"C"}}]"#;
    let categories_body = r#"[{"id":2,"title":"C"}]"#;
    let icon_body = r#"{"id":11,"mime_type":"image/png","data":"data:image/png;base64,AAAA"}"#;
    let server = FakeServer::start(vec![
        (200, counters_body.into()),
        (200, feeds_body.into()),
        (200, categories_body.into()),
        (200, icon_body.into()),
        (204, String::new()),
        (204, String::new()),
    ]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();

    let counters: FeedCountersDto = client.counters().unwrap();
    assert_eq!(counters.unreads.get("3"), Some(&12));

    let feeds = client.feeds().unwrap();
    assert_eq!(feeds[0].category_id(), 2);
    assert_eq!(client.categories().unwrap()[0].id, 2);

    let icon = client.icon(3).unwrap().expect("icon payload");
    assert!(icon.data.starts_with("data:image/png;base64,"));

    client.set_read_batch(&[4, 8], true).unwrap();
    client.toggle_starred(15).unwrap();

    // Drain the queued counters/feeds/categories/icon requests first.
    for _ in 0..4 {
        let _ = server.next_request();
    }
    let mutation = server.next_request();
    assert_eq!(mutation.method, "PUT");
    assert_eq!(mutation.path, "/v1/entries");
    let toggle = server.next_request();
    assert_eq!(toggle.path, "/v1/entries/15/star");
    assert_eq!(toggle.auth_token.as_deref(), Some(KEY));
}

#[test]
fn starred_total_uses_limit_one_query() {
    let server = FakeServer::start(vec![(200, r#"{"total":9,"entries":[]}"#.into())]);
    let client = MinifluxClient::new(&server.base_url, KEY).unwrap();
    assert_eq!(client.starred_total().unwrap(), 9);
    let request = server.next_request();
    assert!(request.path.contains("limit=1"));
    assert!(request.path.contains("starred=1"));
    assert!(request.path.contains("status=read&status=unread"));
}

#[test]
fn browse_filter_construction_matches_go() {
    // All selection: both statuses, ascending ID, 200 pages.
    let all = MinifluxClient::browse_filter(false, false, (0, 0));
    assert_eq!(
        all.statuses,
        vec![STATUS_READ.to_string(), STATUS_UNREAD.to_string()]
    );
    assert_eq!(all.status, None);
    assert_eq!(all.limit, BROWSE_PAGE_SIZE);

    // Unread-only selections collapse to a single status filter.
    let unread = MinifluxClient::browse_filter(true, false, (0, 0));
    assert!(unread.statuses.is_empty());
    assert_eq!(unread.status.as_deref(), Some(STATUS_UNREAD));

    // Starred + feed scope.
    let scoped = MinifluxClient::browse_filter(false, true, (0, 42));
    assert_eq!(scoped.starred.as_deref(), Some(FILTER_ONLY_STARRED));
    assert_eq!(scoped.feed_id, 42);
    assert_eq!(scoped.category_id, 0);

    // Category scope uses category_id, not feed_id.
    let category = MinifluxClient::browse_filter(true, false, (7, 0));
    assert_eq!(category.category_id, 7);
    assert_eq!(category.feed_id, 0);
}
