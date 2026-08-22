//! Test-only SQLite interoperability helper invoked by
//! `Build/test-sqlite-compat.sh`.

use std::path::{Path, PathBuf};

use fluxcore::domain::account::account_id;
use fluxcore::domain::entry::{Entry, EntryStatus};
use fluxcore::domain::navigation::{Category, Feed};
use fluxcore::persistence::{PersistedEntry, Store};

const SERVER: &str = "https://compat.example";
const OTHER_SERVER: &str = "https://compat-other.example";
const API_KEY: &str = "compat-key";
/// Must match fixtureAccount constants in go-core/cmd/sqlite-compat.
const FIXTURE_SERVER: &str = "https://fixture.example";
const FIXTURE_KEY: &str = "fixture-key";

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments.next().expect("mode required");
    if mode == "remote-browse" {
        let base = arguments.next().expect("baseURL required");
        let key = arguments.next().expect("apiKey required");
        let kind = arguments.next().expect("kind required");
        let id: i64 = arguments.next().expect("id required").parse().unwrap();
        let unread_only = arguments.next().expect("unreadOnly required") == "true";
        remote_browse(&base, &key, &kind, id, unread_only);
        return;
    }
    let path = PathBuf::from(arguments.next().expect("database path required"));
    require_temporary_path(&path);

    match mode.as_str() {
        "create" => create(&path),
        "read-go" => read_go(&path),
        "create-mutations" => create_mutations(&path),
        "continue-go-mutations" => continue_mutations(&path, "Go"),
        "snapshot" => {
            // snapshot <db> <kind> <id> <unreadOnly> <retainCSV> <newestFirst>
            let kind = arguments.next().expect("selection kind required");
            let id: i64 = arguments.next().expect("id required").parse().expect("id");
            let unread_only = arguments.next().expect("unreadOnly required") == "true";
            let retain_csv = arguments.next().expect("retainCSV required");
            let newest_first = arguments.next().expect("newestFirst required") == "true";
            snapshot(&path, &kind, id, unread_only, &retain_csv, newest_first);
        }
        "sync-probe" => {
            let base = arguments.next().expect("baseURL required");
            let scenario = arguments.next().expect("scenario required");
            sync_probe(&path, &base, &scenario);
        }
        other => panic!("unsupported mode: {other}"),
    }
}

fn create_mutations(path: &Path) {
    let store = Store::open(path).expect("open Rust mutation store");
    let account = account_id(SERVER, API_KEY);
    let other = account_id(OTHER_SERVER, API_KEY);
    for (id, server, label) in [(&account, SERVER, "A"), (&other, OTHER_SERVER, "B")] {
        store.ensure_account(id, server).unwrap();
        store.apply_snapshot(id, &mutation_snapshot(label)).unwrap();
    }
    store.set_read(&account, &[1], true, true).unwrap();
    store.set_read(&account, &[2], true, true).unwrap();
    store.set_starred(&account, 3, true).unwrap();
    store.set_read(&other, &[1], true, true).unwrap();
}

fn continue_mutations(path: &Path, producer: &str) {
    let account = account_id(SERVER, API_KEY);
    let other = account_id(OTHER_SERVER, API_KEY);
    let store = Store::open(path).expect("open mutation store");
    assert_mutation_snapshot(
        &store,
        &account,
        &[
            "A One:read:false",
            "A Two:read:false",
            "A Three:unread:true",
        ],
    );
    assert_mutation_snapshot(
        &store,
        &other,
        &[
            "B One:read:false",
            "B Two:unread:false",
            "B Three:unread:false",
        ],
    );
    assert_mutation_snapshot(
        &store,
        &account,
        &[
            "A One:read:false",
            "A Two:read:false",
            "A Three:unread:true",
        ],
    );
    let pending = store.pending(&account).unwrap();
    assert_eq!(pending.len(), 3, "{producer} pending state");
    let connection = rusqlite::Connection::open(path).unwrap();
    let mut statement = connection
        .prepare("SELECT entry_id,batch_id FROM undo_items WHERE account_id=?1 ORDER BY entry_id")
        .unwrap();
    let batch_ids: std::collections::HashMap<i64, String> = statement
        .query_map([&account], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    drop(statement);
    drop(connection);
    assert_eq!(batch_ids.len(), 2, "{producer} undo batches");
    for mutation in &pending {
        store.acknowledge(&account, mutation).unwrap();
    }
    store.undo(&account, &batch_ids[&1]).unwrap();
    store.discard_undo(&account, &batch_ids[&2]).unwrap();
    let continued = store.pending(&account).unwrap();
    assert!(
        continued.len() == 1
            && continued[0].entry_id == 1
            && continued[0].field == "read"
            && !continued[0].desired,
        "{producer} continuation: {continued:?}"
    );
    let other_pending = store.pending(&other).unwrap();
    assert!(
        other_pending.len() == 1 && other_pending[0].entry_id == 1 && other_pending[0].desired,
        "{producer} other pending: {other_pending:?}"
    );
    let connection = rusqlite::Connection::open(path).unwrap();
    let account_undo: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM undo_batches WHERE account_id=?1",
            [&account],
            |row| row.get(0),
        )
        .unwrap();
    let other_undo: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM undo_batches WHERE account_id=?1",
            [&other],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        (account_undo, other_undo),
        (0, 1),
        "{producer} undo isolation"
    );
    assert_mutation_snapshot(
        &store,
        &account,
        &[
            "A One:unread:false",
            "A Two:read:false",
            "A Three:unread:true",
        ],
    );
    assert_mutation_snapshot(
        &store,
        &other,
        &[
            "B One:read:false",
            "B Two:unread:false",
            "B Three:unread:false",
        ],
    );
}

fn assert_mutation_snapshot(store: &Store, account: &str, expected: &[&str]) {
    use fluxcore::domain::selection::Selection;
    let snapshot = store
        .local_snapshot(
            account,
            &Selection::All {
                id: 0,
                unread_only: false,
            },
            false,
            &[],
        )
        .unwrap();
    let actual: Vec<String> = snapshot
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}",
                entry.title,
                entry.status.as_str(),
                entry.starred
            )
        })
        .collect();
    assert_eq!(actual, expected, "account snapshot");
}

fn mutation_snapshot(label: &str) -> fluxcore::persistence::SnapshotData {
    use fluxcore::domain::selection::Selection;
    fluxcore::persistence::SnapshotData {
        version: 1,
        selection: Selection::All {
            id: 0,
            unread_only: false,
        },
        entries: vec![
            mutation_entry(label, 1, "2026-08-22T10:00:01Z"),
            mutation_entry(label, 2, "2026-08-22T10:00:02Z"),
            mutation_entry(label, 3, "2026-08-22T10:00:03Z"),
        ],
        categories: vec![Category {
            id: 10,
            title: format!("{label} Category"),
            unread_count: 3,
            feeds: vec![Feed {
                id: 20,
                title: format!("{label} Feed"),
                category_id: 10,
                unread_count: 3,
            }],
        }],
        total: 3,
        unread_total: 3,
        starred_total: 0,
    }
}

fn mutation_entry(label: &str, id: i64, published: &str) -> Entry {
    Entry {
        id,
        title: format!(
            "{label} {}",
            match id {
                1 => "One",
                2 => "Two",
                _ => "Three",
            }
        ),
        url: format!("https://example.com/{id}"),
        comments_url: String::new(),
        feed_id: 20,
        feed_name: format!("{label} Feed"),
        category_id: 10,
        published_at_rfc3339: published.to_string(),
        preview: String::new(),
        image_url: String::new(),
        status: EntryStatus::Unread,
        starred: false,
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncProbeEntry {
    id: i64,
    status: String,
    starred: bool,
}

#[derive(serde::Serialize)]
struct SyncProbePending {
    #[serde(rename = "entryID")]
    entry_id: i64,
    field: String,
    desired: bool,
    revision: i64,
}

#[derive(serde::Serialize)]
struct SyncProbeState {
    label: String,
    entries: Vec<SyncProbeEntry>,
    pending: Vec<SyncProbePending>,
}

fn capture_sync_state(path: &Path, account: &str, label: &str) -> SyncProbeState {
    use fluxcore::domain::selection::Selection;

    let store = Store::open(path).expect("open trace store");
    let snapshot = store
        .local_snapshot(
            account,
            &Selection::All {
                id: 0,
                unread_only: false,
            },
            false,
            &[],
        )
        .expect("trace snapshot");
    let pending = store.pending(account).expect("trace pending");
    SyncProbeState {
        label: label.to_string(),
        entries: snapshot
            .entries
            .into_iter()
            .map(|entry| SyncProbeEntry {
                id: entry.id,
                status: entry.status.as_str().to_string(),
                starred: entry.starred,
            })
            .collect(),
        pending: pending
            .into_iter()
            .map(|mutation| SyncProbePending {
                entry_id: mutation.entry_id,
                field: mutation.field,
                desired: mutation.desired,
                revision: mutation.revision,
            })
            .collect(),
    }
}

fn sync_probe(path: &Path, base: &str, scenario: &str) {
    use fluxcore::domain::selection::Selection;
    use fluxcore::remote::MinifluxClient;
    use fluxcore::sync::{SyncResult, SyncService};

    let account = account_id(base, API_KEY);
    let selection = Selection::All {
        id: 0,
        unread_only: scenario != "incremental",
    };
    let store = Store::open(path).expect("open sync store");
    store
        .ensure_account(&account, base)
        .expect("ensure account");
    let service = SyncService::new(
        store,
        Box::new(MinifluxClient::new(base, API_KEY).expect("remote")),
        account.clone(),
        false,
    );
    let mut error = String::new();
    let mut trace = Vec::new();
    let mut data = match service.sync(&selection, &[]) {
        Ok(SyncResult::Success(data)) => data,
        Ok(SyncResult::Partial(data, problem)) => {
            error = problem;
            data
        }
        Err(problem) => panic!("initial sync failed: {problem}"),
    };
    if matches!(
        scenario,
        "incremental"
            | "incomplete"
            | "refresh-5xx"
            | "refresh-auth"
            | "pagination-duplicate"
            | "pagination-reordered"
            | "pagination-growing-total"
            | "pagination-shrinking-total"
            | "pagination-malformed"
    ) {
        match service.sync(&selection, &[]) {
            Ok(SyncResult::Success(next)) => data = next,
            Ok(SyncResult::Partial(next, problem)) => {
                data = next;
                error = problem;
            }
            Err(problem) => error = problem,
        }
    }
    drop(service);

    if !matches!(
        scenario,
        "initial"
            | "incremental"
            | "incomplete"
            | "refresh-5xx"
            | "refresh-auth"
            | "pagination-duplicate"
            | "pagination-reordered"
            | "pagination-growing-total"
            | "pagination-shrinking-total"
            | "pagination-malformed"
    ) {
        let store = Store::open(path).expect("reopen sync store");
        let mut receipt = match scenario {
            "read" => store.set_read(&account, &[1], true, false).unwrap(),
            "read-reversal" => {
                store.set_read(&account, &[1], true, false).unwrap();
                store.set_read(&account, &[1], false, false).unwrap();
                None
            }
            "star-reversal" => {
                store.set_starred(&account, 1, true).unwrap();
                store.set_starred(&account, 1, false).unwrap();
                None
            }
            "star" => {
                store.set_starred(&account, 1, true).unwrap();
                None
            }
            "read-cycle" => {
                store.set_read(&account, &[1], true, false).unwrap();
                trace.push(capture_sync_state(path, &account, "read"));
                store.set_read(&account, &[1], false, false).unwrap();
                trace.push(capture_sync_state(path, &account, "unread"));
                store.set_read(&account, &[1], true, false).unwrap();
                trace.push(capture_sync_state(path, &account, "read-again"));
                None
            }
            "read-identical" => {
                store.set_read(&account, &[1], true, false).unwrap();
                trace.push(capture_sync_state(path, &account, "read"));
                store.set_read(&account, &[1], true, false).unwrap();
                trace.push(capture_sync_state(path, &account, "read-identical"));
                None
            }
            "star-cycle" => {
                store.set_starred(&account, 1, true).unwrap();
                trace.push(capture_sync_state(path, &account, "starred"));
                store.set_starred(&account, 1, false).unwrap();
                trace.push(capture_sync_state(path, &account, "unstarred"));
                store.set_starred(&account, 1, true).unwrap();
                trace.push(capture_sync_state(path, &account, "starred-again"));
                None
            }
            "star-identical" => {
                store.set_starred(&account, 1, true).unwrap();
                trace.push(capture_sync_state(path, &account, "starred"));
                store.set_starred(&account, 1, true).unwrap();
                trace.push(capture_sync_state(path, &account, "starred-identical"));
                None
            }
            "pending-stale" => {
                store.set_read(&account, &[1], true, false).unwrap();
                store.set_starred(&account, 1, true).unwrap();
                None
            }
            "partial-failure" | "full-failure" => {
                store.set_read(&account, &[1, 2], true, false).unwrap();
                None
            }
            "mixed-middle-retry" => {
                store.set_read(&account, &[1], true, false).unwrap();
                store.set_starred(&account, 1, true).unwrap();
                store.set_read(&account, &[2], true, false).unwrap();
                trace.push(capture_sync_state(path, &account, "queued"));
                None
            }
            "restart-pending" => {
                store.set_read(&account, &[1], true, false).unwrap();
                store.set_starred(&account, 2, true).unwrap();
                trace.push(capture_sync_state(path, &account, "before-restart"));
                None
            }
            "undo-after-flush" => store.set_read(&account, &[1], true, true).unwrap(),
            "undo-before-flush" => {
                let receipt = store.set_read(&account, &[1], true, true).unwrap().unwrap();
                store.undo(&account, &receipt.id).unwrap();
                None
            }
            "discard-undo" => {
                let receipt = store.set_read(&account, &[1], true, true).unwrap().unwrap();
                store.discard_undo(&account, &receipt.id).unwrap();
                None
            }
            _ => panic!("unknown sync scenario"),
        };
        let store = if scenario == "restart-pending" {
            drop(store);
            trace.push(capture_sync_state(path, &account, "after-restart"));
            Store::open(path).expect("restart sync store")
        } else {
            store
        };
        let service = SyncService::new(
            store,
            Box::new(MinifluxClient::new(base, API_KEY).expect("remote")),
            account.clone(),
            false,
        );
        if scenario == "pending-stale" {
            match service.sync(&selection, &[]) {
                Ok(SyncResult::Success(_)) => {}
                Ok(SyncResult::Partial(_, problem)) => error = problem,
                Err(problem) => error = problem,
            }
        } else if scenario == "mixed-middle-retry" {
            if service.flush().is_ok() {
                panic!("mixed middle flush unexpectedly succeeded");
            }
            trace.push(capture_sync_state(path, &account, "middle-failed"));
            if let Err(problem) = service.flush() {
                error = problem;
            }
            trace.push(capture_sync_state(path, &account, "retried"));
        } else if let Err(problem) = service.flush() {
            error = problem;
        } else if scenario == "undo-after-flush" {
            if let Some(receipt) = receipt.take() {
                if let Err(problem) = service.undo(&selection, &receipt.id, &[]) {
                    error = problem;
                } else if let Err(problem) = service.flush() {
                    error = problem;
                }
            } else {
                panic!("missing undo receipt")
            }
        }
        data = service
            .local_snapshot(&selection, &[])
            .expect("final local snapshot");
    }

    let output = serde_json::json!({
        "error": if error.is_empty() { serde_json::Value::Null } else { error.into() },
        "snapshot": fluxcore::snapshot::assemble(&data),
        "trace": trace,
    });
    println!("{}", serde_json::to_string(&output).unwrap());
}

/// Prints the Rust local snapshot for the fixture account as JSON.
fn snapshot(
    path: &Path,
    kind: &str,
    id: i64,
    unread_only: bool,
    retain_csv: &str,
    newest_first: bool,
) {
    let store = Store::open(path).expect("open fixture store");
    let account = account_id(FIXTURE_SERVER, FIXTURE_KEY);
    let retain_ids: Vec<i64> = retain_csv
        .split(',')
        .filter_map(|field| field.trim().parse::<i64>().ok())
        .collect();
    let selection = fluxcore::domain::selection::Selection::normalize(kind, id, unread_only);
    let data = store
        .local_snapshot(&account, &selection, newest_first, &retain_ids)
        .expect("local snapshot");
    println!(
        "{}",
        serde_json::to_string(&fluxcore::snapshot::assemble(&data)).unwrap()
    );
}

fn create(path: &Path) {
    let store = Store::open(path).expect("open Rust store");
    let account = account_id(SERVER, API_KEY);
    store
        .ensure_account(&account, SERVER)
        .expect("ensure account");
    store
        .upsert_category(
            &account,
            &Category {
                id: 10,
                title: "Rust Category".to_string(),
                unread_count: 4,
                feeds: Vec::new(),
            },
        )
        .expect("upsert category");
    store
        .upsert_feed(
            &account,
            &Feed {
                id: 20,
                title: "Rust Feed".to_string(),
                category_id: 10,
                unread_count: 4,
            },
        )
        .expect("upsert feed");
    store
        .upsert_selection_total(&account, "all", 0, false, 1)
        .expect("upsert total");
    store
        .upsert_entry(
            &account,
            &PersistedEntry {
                entry: Entry {
                    id: 30,
                    title: "Rust Entry".to_string(),
                    url: "https://example.com/rust".to_string(),
                    comments_url: String::new(),
                    feed_id: 20,
                    feed_name: "Rust Feed".to_string(),
                    category_id: 10,
                    published_at_rfc3339: "2026-08-22T12:34:56.123456789Z".to_string(),
                    preview: "Rust preview".to_string(),
                    image_url: String::new(),
                    status: EntryStatus::Other("rust-future-status".to_string()),
                    starred: true,
                },
                remote_status: EntryStatus::Other("rust-future-status".to_string()),
                remote_starred: true,
            },
        )
        .expect("upsert entry");
}

fn read_go(path: &Path) {
    let store = Store::open(path).expect("open Go-created store");
    let account = account_id(SERVER, API_KEY);
    let record = store
        .entry(&account, 30)
        .expect("read Go entry")
        .expect("Go entry missing");
    assert_eq!(record.entry.title, "Go Entry");
    assert_eq!(record.entry.status.as_str(), "go-future-status");
    assert_eq!(record.remote_status.as_str(), "go-future-status");
    assert_eq!(
        record.entry.published_at_rfc3339,
        "2026-08-22T12:34:56.123456789Z"
    );
}

/// Mirrors Go `Service.Browse` using the Rust remote adapter + domain
/// navigation; output must be JSON-identical to the Go probe.
fn remote_browse(base: &str, key: &str, kind: &str, id: i64, unread_only: bool) {
    use fluxcore::domain::navigation::build_navigation;
    use fluxcore::remote::RemoteInbox;
    use fluxcore::remote::miniflux::MinifluxClient;

    let client = MinifluxClient::new(base, key).expect("client");
    let selection = fluxcore::domain::selection::Selection::normalize(kind, id, unread_only);
    let starred_selection = matches!(
        selection,
        fluxcore::domain::selection::Selection::Starred { .. }
    );
    let category_id = match selection {
        fluxcore::domain::selection::Selection::Category { id, .. } => id,
        _ => 0,
    };
    let feed_id = match selection {
        fluxcore::domain::selection::Selection::Feed { id, .. } => id,
        _ => 0,
    };

    let counter_map: std::collections::HashMap<i64, i32> = client
        .unread_counters()
        .expect("counters")
        .unreads
        .iter()
        .filter_map(|(key, value)| key.parse::<i64>().ok().map(|key| (key, *value)))
        .collect();

    let starred_total = if starred_selection {
        0 // filled after fetch below
    } else {
        client.starred_total().expect("starred total")
    };

    let filter = MinifluxClient::browse_filter(
        selection.is_unread_only(),
        starred_selection,
        (category_id, feed_id),
    );
    let (entries, total) = client.fetch_complete_selection(&filter).expect("browse");
    let starred_total = if starred_selection {
        total
    } else {
        starred_total
    };
    let categories = client.categories().expect("categories");
    let feeds = client.feeds().expect("feeds");

    let (category_inputs, feed_inputs) = fluxcore::remote::dto_mapping_inputs(&categories, &feeds);
    let (navigation, unread_total) = build_navigation(&category_inputs, &feed_inputs, &counter_map);

    let domain_entries: Vec<fluxcore::domain::entry::Entry> = entries
        .iter()
        .map(fluxcore::remote::entry_to_domain)
        .collect();

    let data = fluxcore::persistence::SnapshotData {
        version: 1,
        selection,
        entries: domain_entries,
        categories: navigation,
        total: total as i32,
        unread_total,
        starred_total: starred_total as i32,
    };
    println!(
        "{}",
        serde_json::to_string(&fluxcore::snapshot::assemble(&data)).unwrap()
    );
}

fn require_temporary_path(path: &Path) {
    let temporary = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temp dir");
    let parent = path.parent().expect("database parent");
    let parent = parent.canonicalize().expect("canonical database parent");
    assert!(
        parent.starts_with(&temporary),
        "refusing non-temporary database path: {}",
        path.display()
    );
}
