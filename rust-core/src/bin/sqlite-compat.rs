//! Test-only SQLite interoperability helper invoked by
//! `Build/test-sqlite-compat.sh`.

use std::path::{Path, PathBuf};

use fluxcore::domain::account::account_id;
use fluxcore::domain::entry::{Entry, EntryStatus};
use fluxcore::domain::navigation::{Category, Feed};
use fluxcore::persistence::{PersistedEntry, Store};

const SERVER: &str = "https://compat.example";
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
            // snapshot <db> <kind> <id> <unreadOnly> <retainCSV>
            let kind = arguments.next().expect("selection kind required");
            let id: i64 = arguments.next().expect("id required").parse().expect("id");
            let unread_only = arguments.next().expect("unreadOnly required") == "true";
            let retain_csv = arguments.next().expect("retainCSV required");
            snapshot(&path, &kind, id, unread_only, &retain_csv);
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
    store.ensure_account(&account, SERVER).unwrap();
    store
        .apply_snapshot(&account, &mutation_snapshot())
        .unwrap();
    store.set_read(&account, &[1], true, true).unwrap();
    store.set_starred(&account, 2, true).unwrap();
}

fn continue_mutations(path: &Path, producer: &str) {
    let account = account_id(SERVER, API_KEY);
    let batch_id: String = rusqlite::Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT id FROM undo_batches WHERE account_id=?1",
            [&account],
            |row| row.get(0),
        )
        .unwrap();
    let store = Store::open(path).expect("open mutation store");
    let pending = store.pending(&account).unwrap();
    assert_eq!(pending.len(), 2, "{producer} pending state");
    store.undo(&account, &batch_id).unwrap();
    assert_eq!(
        store.pending(&account).unwrap().len(),
        2,
        "{producer} continuation"
    );
}

fn mutation_snapshot() -> fluxcore::persistence::SnapshotData {
    use fluxcore::domain::selection::Selection;
    fluxcore::persistence::SnapshotData {
        version: 1,
        selection: Selection::All {
            id: 0,
            unread_only: true,
        },
        entries: vec![
            mutation_entry(1, "2026-08-22T10:00:01Z"),
            mutation_entry(2, "2026-08-22T10:00:02Z"),
        ],
        categories: vec![Category {
            id: 10,
            title: "Category".to_string(),
            unread_count: 2,
            feeds: vec![Feed {
                id: 20,
                title: "Feed".to_string(),
                category_id: 10,
                unread_count: 2,
            }],
        }],
        total: 2,
        unread_total: 2,
        starred_total: 0,
    }
}

fn mutation_entry(id: i64, published: &str) -> Entry {
    Entry {
        id,
        title: if id == 1 { "One" } else { "Two" }.to_string(),
        url: format!("https://example.com/{id}"),
        comments_url: String::new(),
        feed_id: 20,
        feed_name: "Feed".to_string(),
        category_id: 10,
        published_at_rfc3339: published.to_string(),
        preview: String::new(),
        image_url: String::new(),
        status: EntryStatus::Unread,
        starred: false,
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
        "incremental" | "incomplete" | "refresh-5xx" | "refresh-auth"
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
        "initial" | "incremental" | "incomplete" | "refresh-5xx" | "refresh-auth"
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
            "pending-stale" => {
                store.set_read(&account, &[1], true, false).unwrap();
                store.set_starred(&account, 1, true).unwrap();
                None
            }
            "partial-failure" | "full-failure" => {
                store.set_read(&account, &[1, 2], true, false).unwrap();
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
        let service = SyncService::new(
            store,
            Box::new(MinifluxClient::new(base, API_KEY).expect("remote")),
            account,
            false,
        );
        if scenario == "pending-stale" {
            match service.sync(&selection, &[]) {
                Ok(SyncResult::Success(_)) => {}
                Ok(SyncResult::Partial(_, problem)) => error = problem,
                Err(problem) => error = problem,
            }
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
    });
    println!("{}", serde_json::to_string(&output).unwrap());
}

/// Prints the Rust local snapshot for the fixture account as JSON.
fn snapshot(path: &Path, kind: &str, id: i64, unread_only: bool, retain_csv: &str) {
    let store = Store::open(path).expect("open fixture store");
    let account = account_id(FIXTURE_SERVER, FIXTURE_KEY);
    let retain_ids: Vec<i64> = retain_csv
        .split(',')
        .filter_map(|field| field.trim().parse::<i64>().ok())
        .collect();
    let selection = fluxcore::domain::selection::Selection::normalize(kind, id, unread_only);
    let data = store
        .local_snapshot(&account, &selection, false, &retain_ids)
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
