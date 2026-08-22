//! Go-compatible sync and mutation orchestration.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::domain::navigation::build_navigation;
use crate::domain::selection::Selection;
use crate::persistence::{MutationReceipt, SnapshotData, Store};
use crate::remote::miniflux::{BROWSE_PAGE_SIZE, FILTER_ONLY_STARRED, STATUS_READ, STATUS_UNREAD};
use crate::remote::{EntriesFilter, RemoteInbox, dto_mapping_inputs, entry_to_domain};

pub const AUTOMATIC_FLUSH_DELAY: Duration = Duration::from_secs(10);
pub const REFRESH_DEADLINE: Duration = Duration::from_secs(45);
pub const FLUSH_DEADLINE: Duration = Duration::from_secs(30);

struct Inner {
    store: Store,
    remote: Box<dyn RemoteInbox>,
    account_id: String,
    newest_first: bool,
}

#[derive(Default)]
struct ScheduleState {
    deadline: Option<Instant>,
    worker_running: bool,
}

#[derive(Default)]
struct Scheduler {
    state: Mutex<ScheduleState>,
    changed: Condvar,
}

/// One account-bound service. All store/remote operations are serialized by a
/// single mutex, which is stricter than Go's DB+sync locks but preserves their
/// externally observable ordering and prevents SQLite use from timer threads.
pub struct SyncService {
    inner: Mutex<Inner>,
    scheduler: Scheduler,
    automatic_flush_delay: Duration,
}

pub enum SyncResult {
    Success(SnapshotData),
    Partial(SnapshotData, String),
}

impl SyncService {
    pub fn new(
        store: Store,
        remote: Box<dyn RemoteInbox>,
        account_id: String,
        newest_first: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                store,
                remote,
                account_id,
                newest_first,
            }),
            scheduler: Scheduler::default(),
            automatic_flush_delay: AUTOMATIC_FLUSH_DELAY,
        })
    }

    #[cfg(test)]
    fn new_with_delay(
        store: Store,
        remote: Box<dyn RemoteInbox>,
        account_id: String,
        newest_first: bool,
        automatic_flush_delay: Duration,
    ) -> Arc<Self> {
        let mut service = Arc::try_unwrap(Self::new(store, remote, account_id, newest_first))
            .ok()
            .expect("new service is uniquely owned");
        service.automatic_flush_delay = automatic_flush_delay;
        Arc::new(service)
    }

    pub fn local_snapshot(
        &self,
        selection: &Selection,
        retain_ids: &[i64],
    ) -> Result<SnapshotData, String> {
        let inner = locked(&self.inner);
        local_snapshot(&inner, selection, retain_ids)
    }

    pub fn sync(&self, selection: &Selection, retain_ids: &[i64]) -> Result<SyncResult, String> {
        let mut inner = locked(&self.inner);
        inner
            .remote
            .set_operation_deadline(Some(Instant::now() + REFRESH_DEADLINE));
        let result = sync_locked(&mut inner, selection, retain_ids);
        inner.remote.set_operation_deadline(None);
        result
    }

    pub fn flush(&self) -> Result<(), String> {
        let mut inner = locked(&self.inner);
        inner
            .remote
            .set_operation_deadline(Some(Instant::now() + FLUSH_DEADLINE));
        let result = flush_pending(&mut inner);
        inner.remote.set_operation_deadline(None);
        result
    }

    /*
     * Mutation methods below are local-only and bounded by SQLite's 5-second
     * busy timeout. Their scheduled remote work uses the 30-second flush
     * deadline independently.
     */

    pub fn mark_read(
        self: &Arc<Self>,
        selection: &Selection,
        ids: &[i64],
        retain_ids: &[i64],
        read: bool,
        automatic: bool,
    ) -> Result<(SnapshotData, Option<MutationReceipt>), String> {
        let (snapshot, receipt) = {
            let inner = locked(&self.inner);
            let receipt = inner
                .store
                .set_read(&inner.account_id, ids, read, automatic)
                .map_err(|error| error.to_string())?;
            let mut retained = retain_ids.to_vec();
            retained.extend_from_slice(ids);
            let snapshot = local_snapshot(&inner, selection, &retained)?;
            (snapshot, receipt)
        };
        self.schedule_flush(if automatic {
            self.automatic_flush_delay
        } else {
            Duration::ZERO
        });
        Ok((snapshot, receipt))
    }

    pub fn set_starred(
        self: &Arc<Self>,
        selection: &Selection,
        entry_id: i64,
        starred: bool,
        retain_ids: &[i64],
    ) -> Result<SnapshotData, String> {
        let snapshot = {
            let inner = locked(&self.inner);
            inner
                .store
                .set_starred(&inner.account_id, entry_id, starred)
                .map_err(|error| error.to_string())?;
            let mut retained = retain_ids.to_vec();
            retained.push(entry_id);
            local_snapshot(&inner, selection, &retained)?
        };
        self.schedule_flush(Duration::ZERO);
        Ok(snapshot)
    }

    pub fn undo(
        self: &Arc<Self>,
        selection: &Selection,
        receipt_id: &str,
        retain_ids: &[i64],
    ) -> Result<SnapshotData, String> {
        let snapshot = {
            let inner = locked(&self.inner);
            let ids = inner
                .store
                .undo(&inner.account_id, receipt_id)
                .map_err(|error| error.to_string())?;
            let mut retained = retain_ids.to_vec();
            retained.extend(ids);
            local_snapshot(&inner, selection, &retained)?
        };
        self.schedule_flush(Duration::ZERO);
        Ok(snapshot)
    }

    pub fn discard_undo(&self, receipt_id: &str) -> Result<(), String> {
        let inner = locked(&self.inner);
        inner
            .store
            .discard_undo(&inner.account_id, receipt_id)
            .map_err(|error| error.to_string())
    }

    fn schedule_flush(self: &Arc<Self>, delay: Duration) {
        let mut schedule = locked(&self.scheduler.state);
        schedule.deadline = Some(Instant::now() + delay);
        self.scheduler.changed.notify_one();
        if schedule.worker_running {
            return;
        }
        schedule.worker_running = true;
        let service = Arc::clone(self);
        std::thread::spawn(move || service.run_scheduler());
    }

    fn run_scheduler(self: Arc<Self>) {
        loop {
            let mut schedule = locked(&self.scheduler.state);
            let Some(deadline) = schedule.deadline else {
                schedule.worker_running = false;
                return;
            };
            let now = Instant::now();
            if now < deadline {
                let duration = deadline.saturating_duration_since(now);
                let (next, _) = self
                    .scheduler
                    .changed
                    .wait_timeout(schedule, duration)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                drop(next);
                continue;
            }
            schedule.deadline = None;
            drop(schedule);
            // Background errors are intentionally non-fatal, as in Go's
            // logging-only timer callback. A panic remains contained to this
            // worker and cannot unwind across the C ABI.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.flush()));
        }
    }
}

fn sync_locked(
    inner: &mut Inner,
    selection: &Selection,
    retain_ids: &[i64],
) -> Result<SyncResult, String> {
    // Go logs and ignores a pre-refresh flush error.
    let _ = flush_pending(inner);
    let remote_snapshot = match browse(inner.remote.as_ref(), selection) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return local_snapshot(inner, selection, retain_ids)
                .map(|snapshot| SyncResult::Partial(snapshot, error));
        }
    };
    inner
        .store
        .apply_snapshot(&inner.account_id, &remote_snapshot)
        .map_err(|error| error.to_string())?;
    local_snapshot(inner, selection, retain_ids).map(SyncResult::Success)
}

fn local_snapshot(
    inner: &Inner,
    selection: &Selection,
    retain_ids: &[i64],
) -> Result<SnapshotData, String> {
    inner
        .store
        .local_snapshot(&inner.account_id, selection, inner.newest_first, retain_ids)
        .map_err(|error| error.to_string())
}

fn browse(remote: &dyn RemoteInbox, selection: &Selection) -> Result<SnapshotData, String> {
    let counters = remote
        .unread_counters()
        .map_err(|error| format!("Miniflux-Zähler laden: {error}"))?;
    let mut starred_total = 0;
    if !matches!(selection, Selection::Starred { .. }) {
        starred_total = remote
            .starred_total()
            .map_err(|error| format!("markierte Miniflux-Einträge zählen: {error}"))?
            as i32;
    }
    let filter = browse_filter(selection);
    let (entries, total) = remote
        .fetch_complete_selection(&filter)
        .map_err(|error| format!("Miniflux-Einträge laden: {error}"))?;
    if matches!(selection, Selection::Starred { .. }) {
        starred_total = total as i32;
    }
    let categories = remote
        .categories()
        .map_err(|error| format!("Miniflux-Kategorien laden: {error}"))?;
    let feeds = remote
        .feeds()
        .map_err(|error| format!("Miniflux-Feeds laden: {error}"))?;
    let (category_inputs, feed_inputs) = dto_mapping_inputs(&categories, &feeds);
    let unread_counters: HashMap<i64, i32> = counters
        .unreads
        .into_iter()
        .filter_map(|(id, count)| id.parse().ok().map(|id| (id, count)))
        .collect();
    let (navigation, unread_total) =
        build_navigation(&category_inputs, &feed_inputs, &unread_counters);
    Ok(SnapshotData {
        version: 1,
        selection: selection.clone(),
        entries: entries.iter().map(entry_to_domain).collect(),
        categories: navigation,
        total: total as i32,
        unread_total,
        starred_total,
    })
}

fn browse_filter(selection: &Selection) -> EntriesFilter {
    let mut filter = EntriesFilter {
        limit: BROWSE_PAGE_SIZE,
        offset: 0,
        order: Some("id".to_string()),
        direction: Some("asc".to_string()),
        statuses: vec![STATUS_READ.to_string(), STATUS_UNREAD.to_string()],
        ..Default::default()
    };
    if selection.is_unread_only() {
        filter.statuses.clear();
        filter.status = Some(STATUS_UNREAD.to_string());
    }
    match selection {
        Selection::Starred { .. } => filter.starred = Some(FILTER_ONLY_STARRED.to_string()),
        Selection::Category { id, .. } => filter.category_id = *id,
        Selection::Feed { id, .. } => filter.feed_id = *id,
        _ => {}
    }
    filter
}

fn flush_pending(inner: &mut Inner) -> Result<(), String> {
    let pending = inner
        .store
        .pending(&inner.account_id)
        .map_err(|error| error.to_string())?;
    for mutation in pending {
        match mutation.field.as_str() {
            "read" => inner
                .remote
                .set_read_batch(&[mutation.entry_id], mutation.desired)
                .map_err(|error| format!("Miniflux-Lesestatus aktualisieren: {error}"))?,
            "starred" => {
                let current = inner
                    .remote
                    .entry_starred(mutation.entry_id)
                    .map_err(|error| format!("Miniflux-Eintrag laden: {error}"))?;
                if current != mutation.desired {
                    inner
                        .remote
                        .toggle_starred(mutation.entry_id)
                        .map_err(|error| format!("Miniflux-Markierung aktualisieren: {error}"))?;
                }
            }
            field => return Err(format!("unbekanntes Mutationsfeld {field:?}")),
        }
        inner
            .store
            .acknowledge(&inner.account_id, &mutation)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::{CategoryDto, EntryDto, FeedCountersDto, FeedDto, RemoteError};

    #[derive(Default)]
    struct FakeState {
        entries: Vec<EntryDto>,
        categories: Vec<CategoryDto>,
        feeds: Vec<FeedDto>,
        counters: HashMap<String, i32>,
        starred_total: i64,
        browse_error: Option<RemoteError>,
        read_error_at: usize,
        read_calls: Vec<(i64, bool)>,
        starred: HashMap<i64, bool>,
        toggle_calls: Vec<i64>,
    }

    #[derive(Clone)]
    struct FakeRemote(Arc<Mutex<FakeState>>);

    impl RemoteInbox for FakeRemote {
        fn fetch_complete_selection(
            &self,
            _filter: &EntriesFilter,
        ) -> Result<(Vec<EntryDto>, i64), RemoteError> {
            let state = locked(&self.0);
            if let Some(error) = &state.browse_error {
                return Err(error.clone());
            }
            Ok((state.entries.clone(), state.entries.len() as i64))
        }

        fn categories(&self) -> Result<Vec<CategoryDto>, RemoteError> {
            Ok(locked(&self.0).categories.clone())
        }

        fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError> {
            Ok(locked(&self.0).feeds.clone())
        }

        fn unread_counters(&self) -> Result<FeedCountersDto, RemoteError> {
            Ok(FeedCountersDto {
                unreads: locked(&self.0).counters.clone(),
            })
        }

        fn starred_total(&self) -> Result<i64, RemoteError> {
            Ok(locked(&self.0).starred_total)
        }

        fn icon_data_url(&self, _feed_id: i64) -> Result<Option<String>, RemoteError> {
            Ok(None)
        }

        fn set_read_batch(&self, ids: &[i64], read: bool) -> Result<(), RemoteError> {
            let mut state = locked(&self.0);
            state.read_calls.push((ids[0], read));
            if state.read_error_at > 0 && state.read_calls.len() == state.read_error_at {
                return Err(RemoteError::Transport("read failed".to_string()));
            }
            Ok(())
        }

        fn entry_starred(&self, entry_id: i64) -> Result<bool, RemoteError> {
            Ok(locked(&self.0)
                .starred
                .get(&entry_id)
                .copied()
                .unwrap_or(false))
        }

        fn toggle_starred(&self, entry_id: i64) -> Result<(), RemoteError> {
            let mut state = locked(&self.0);
            let current = state.starred.get(&entry_id).copied().unwrap_or(false);
            state.starred.insert(entry_id, !current);
            state.toggle_calls.push(entry_id);
            Ok(())
        }
    }

    fn remote_fixture() -> (FakeRemote, Arc<Mutex<FakeState>>) {
        let category = CategoryDto {
            id: 10,
            title: "Category".to_string(),
        };
        let feed = FeedDto {
            id: 20,
            title: "Feed".to_string(),
            category: Some(category.clone()),
        };
        let state = Arc::new(Mutex::new(FakeState {
            entries: vec![entry(1, false), entry(2, false)],
            categories: vec![category],
            feeds: vec![feed],
            counters: HashMap::from([("20".to_string(), 2)]),
            ..Default::default()
        }));
        (FakeRemote(Arc::clone(&state)), state)
    }

    fn entry(id: i64, starred: bool) -> EntryDto {
        EntryDto {
            id,
            feed_id: 20,
            title: format!("Entry {id}"),
            url: format!("https://example.com/{id}"),
            comments_url: String::new(),
            status: "unread".to_string(),
            starred,
            published_at: format!("2026-08-22T10:00:{id:02}Z"),
            content: String::new(),
            enclosures: Vec::new(),
            feed: Some(FeedDto {
                id: 20,
                title: "Feed".to_string(),
                category: Some(CategoryDto {
                    id: 10,
                    title: "Category".to_string(),
                }),
            }),
        }
    }

    fn service(remote: FakeRemote, delay: Duration) -> (TestDirectory, Arc<SyncService>) {
        let directory = TestDirectory::new();
        let store = Store::open(directory.path().join("inbox.sqlite3")).unwrap();
        store
            .ensure_account("account", "https://example.com")
            .unwrap();
        let service = SyncService::new_with_delay(
            store,
            Box::new(remote),
            "account".to_string(),
            false,
            delay,
        );
        (directory, service)
    }

    fn unread_selection() -> Selection {
        Selection::All {
            id: 0,
            unread_only: true,
        }
    }

    #[test]
    fn initial_incremental_and_complete_negative_reconciliation() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        let first = match service.sync(&unread_selection(), &[]).unwrap() {
            SyncResult::Success(snapshot) => snapshot,
            SyncResult::Partial(_, error) => panic!("unexpected partial: {error}"),
        };
        assert_eq!(first.entries.len(), 2);

        {
            let mut state = locked(&state);
            state.entries.remove(0);
            state.entries[0].title = "Updated".to_string();
            state.counters.insert("20".to_string(), 1);
        }
        let second = match service.sync(&unread_selection(), &[]).unwrap() {
            SyncResult::Success(snapshot) => snapshot,
            SyncResult::Partial(_, error) => panic!("unexpected partial: {error}"),
        };
        assert_eq!(second.entries.len(), 1);
        assert_eq!(second.entries[0].id, 2);
        assert_eq!(second.entries[0].title, "Updated");
    }

    #[test]
    fn failed_complete_fetch_returns_local_partial_without_reconciliation() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();
        locked(&state).browse_error = Some(RemoteError::Transport(
            "unvollständige paginierte Antwort".to_string(),
        ));
        match service.sync(&unread_selection(), &[]).unwrap() {
            SyncResult::Partial(snapshot, error) => {
                assert_eq!(snapshot.entries.len(), 2);
                assert!(error.contains("unvollständige"));
            }
            SyncResult::Success(_) => panic!("incomplete pagination unexpectedly succeeded"),
        }
    }

    #[test]
    fn stale_remote_refresh_preserves_pending_read_and_star_desired_state() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();
        locked(&state).read_error_at = 1;
        {
            let inner = locked(&service.inner);
            inner.store.set_read("account", &[1], true, false).unwrap();
            inner.store.set_starred("account", 1, true).unwrap();
        }
        let snapshot = match service.sync(
            &Selection::All {
                id: 0,
                unread_only: false,
            },
            &[],
        ) {
            Ok(SyncResult::Success(snapshot)) => snapshot,
            Ok(SyncResult::Partial(snapshot, _)) => snapshot,
            Err(error) => panic!("sync failed: {error}"),
        };
        let first = snapshot.entries.iter().find(|entry| entry.id == 1).unwrap();
        assert_eq!(first.status.as_str(), "read");
        assert!(first.starred);
    }

    #[test]
    fn flush_acknowledges_prefix_and_leaves_failed_suffix() {
        let (remote, state) = remote_fixture();
        let directory = TestDirectory::new();
        let store = Store::open(directory.path().join("inbox.sqlite3")).unwrap();
        store
            .ensure_account("account", "https://example.com")
            .unwrap();
        let bootstrap = SyncService::new(
            store,
            Box::new(remote.clone()),
            "account".to_string(),
            false,
        );
        bootstrap.sync(&unread_selection(), &[]).unwrap();
        {
            let inner = locked(&bootstrap.inner);
            inner
                .store
                .set_read("account", &[1, 2], true, false)
                .unwrap();
        }
        locked(&state).read_error_at = 2;
        assert!(bootstrap.flush().unwrap_err().contains("read failed"));
        let inner = locked(&bootstrap.inner);
        let pending = inner.store.pending("account").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].entry_id, 2);
        assert_eq!(
            inner
                .store
                .entry("account", 1)
                .unwrap()
                .unwrap()
                .remote_status
                .as_str(),
            "read"
        );
        assert_eq!(
            inner
                .store
                .entry("account", 2)
                .unwrap()
                .unwrap()
                .remote_status
                .as_str(),
            "unread"
        );
    }

    #[test]
    fn undo_before_delay_replaces_pending_with_compensating_state() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_millis(200));
        service.sync(&unread_selection(), &[]).unwrap();
        let (_, receipt) = service
            .mark_read(&unread_selection(), &[1], &[], true, true)
            .unwrap();
        service
            .undo(&unread_selection(), &receipt.unwrap().id, &[])
            .unwrap();
        wait_until(|| locked(&state).read_calls.len() == 1);
        assert_eq!(locked(&state).read_calls, vec![(1, false)]);
        assert!(
            locked(&service.inner)
                .store
                .pending("account")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn delayed_worker_remains_bound_to_its_account_service() {
        let (remote_a, state_a) = remote_fixture();
        let (remote_b, state_b) = remote_fixture();
        let (_directory_a, service_a) = service(remote_a, Duration::from_millis(20));
        let (_directory_b, service_b) = service(remote_b, Duration::from_millis(20));
        service_a.sync(&unread_selection(), &[]).unwrap();
        service_b.sync(&unread_selection(), &[]).unwrap();
        service_a
            .mark_read(&unread_selection(), &[1], &[], true, true)
            .unwrap();
        wait_until(|| locked(&state_a).read_calls.len() == 1);
        assert_eq!(locked(&state_a).read_calls, vec![(1, true)]);
        assert!(locked(&state_b).read_calls.is_empty());
    }

    fn wait_until(predicate: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !predicate() {
            assert!(Instant::now() < deadline, "condition timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "fluxbar-sync-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
