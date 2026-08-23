//! Go-compatible sync and mutation orchestration.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::domain::navigation::build_navigation;
use crate::domain::selection::Selection;
use crate::icons::IconService;
use crate::persistence::{MutationReceipt, SnapshotData, Store};
use crate::remote::miniflux::{BROWSE_PAGE_SIZE, FILTER_ONLY_STARRED, STATUS_READ, STATUS_UNREAD};
use crate::remote::{EntriesFilter, RemoteInbox, dto_mapping_inputs, entry_to_domain};

pub const AUTOMATIC_FLUSH_DELAY: Duration = Duration::from_secs(10);
pub const REFRESH_DEADLINE: Duration = Duration::from_secs(45);
pub const FLUSH_DEADLINE: Duration = Duration::from_secs(30);
pub const ICON_DEADLINE: Duration = Duration::from_secs(15);
pub const LOCAL_DEADLINE: Duration = Duration::from_secs(5);

struct StateLock<T> {
    value: Mutex<Option<T>>,
    available: Condvar,
}

pub(crate) struct SharedStore {
    state: StateLock<Store>,
}

impl SharedStore {
    pub(crate) fn new(store: Store) -> Arc<Self> {
        Arc::new(Self {
            state: StateLock::new(store),
        })
    }

    fn lock_until(&self, deadline: Instant) -> Result<StateGuard<'_, Store>, String> {
        self.state.lock_until(deadline)
    }

    pub(crate) fn ensure_account(&self, account_id: &str, server: &str) -> Result<(), String> {
        self.state
            .lock()
            .ensure_account(account_id, server)
            .map_err(|error| error.to_string())
    }

    #[cfg(test)]
    fn lock(&self) -> StateGuard<'_, Store> {
        self.state.lock()
    }
}

impl<T> StateLock<T> {
    fn new(value: T) -> Self {
        Self {
            value: Mutex::new(Some(value)),
            available: Condvar::new(),
        }
    }

    fn lock(&self) -> StateGuard<'_, T> {
        let mut slot = locked(&self.value);
        while slot.is_none() {
            slot = self
                .available
                .wait(slot)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        StateGuard {
            owner: self,
            value: slot.take(),
        }
    }

    fn lock_until(&self, deadline: Instant) -> Result<StateGuard<'_, T>, String> {
        let mut slot = locked(&self.value);
        while slot.is_none() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("context deadline exceeded".to_string());
            }
            let (next, timeout) = self
                .available
                .wait_timeout(slot, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            slot = next;
            if timeout.timed_out() && slot.is_none() {
                return Err("context deadline exceeded".to_string());
            }
        }
        Ok(StateGuard {
            owner: self,
            value: slot.take(),
        })
    }
}

struct StateGuard<'a, T> {
    owner: &'a StateLock<T>,
    value: Option<T>,
}

impl<T> Deref for StateGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref().expect("state guard owns value")
    }
}

impl<T> DerefMut for StateGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.as_mut().expect("state guard owns value")
    }
}

impl<T> Drop for StateGuard<'_, T> {
    fn drop(&mut self) {
        let mut slot = locked(&self.owner.value);
        *slot = self.value.take();
        self.owner.available.notify_one();
    }
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

/// One account-bound service. SQLite access and refresh/flush orchestration are
/// synchronized independently so local work never waits on remote I/O.
pub struct SyncService {
    store: Arc<SharedStore>,
    remote: Box<dyn RemoteInbox>,
    account_id: String,
    newest_first: AtomicBool,
    icon_service: IconService,
    sync_gate: StateLock<()>,
    scheduler: Scheduler,
    automatic_flush_delay: Duration,
}

struct RemoteDeadlineGuard<'a> {
    remote: &'a dyn RemoteInbox,
}

impl<'a> RemoteDeadlineGuard<'a> {
    fn new(remote: &'a dyn RemoteInbox, deadline: Instant) -> Self {
        remote.set_operation_deadline(Some(deadline));
        Self { remote }
    }
}

impl Drop for RemoteDeadlineGuard<'_> {
    fn drop(&mut self) {
        self.remote.set_operation_deadline(None);
    }
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
        Self::with_shared_store(SharedStore::new(store), remote, account_id, newest_first)
    }

    pub(crate) fn with_shared_store(
        store: Arc<SharedStore>,
        remote: Box<dyn RemoteInbox>,
        account_id: String,
        newest_first: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            remote,
            account_id,
            newest_first: AtomicBool::new(newest_first),
            icon_service: IconService::new(),
            sync_gate: StateLock::new(()),
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
        self.local_snapshot_until(
            selection,
            retain_ids,
            Instant::now() + LOCAL_DEADLINE,
            self.newest_first.load(Ordering::Relaxed),
        )
    }

    pub fn set_newest_first(&self, newest_first: bool) {
        self.newest_first.store(newest_first, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn shared_store(&self) -> Arc<SharedStore> {
        Arc::clone(&self.store)
    }

    pub fn sync(&self, selection: &Selection, retain_ids: &[i64]) -> Result<SyncResult, String> {
        let deadline = Instant::now() + REFRESH_DEADLINE;
        let newest_first = self.newest_first.load(Ordering::Relaxed);
        let _sync = self.sync_gate.lock_until(deadline)?;
        let _deadline = RemoteDeadlineGuard::new(self.remote.as_ref(), deadline);
        self.sync_serialized(selection, retain_ids, deadline, newest_first)
    }

    pub fn flush(&self) -> Result<(), String> {
        let deadline = Instant::now() + FLUSH_DEADLINE;
        let _sync = self.sync_gate.lock_until(deadline)?;
        let _deadline = RemoteDeadlineGuard::new(self.remote.as_ref(), deadline);
        self.flush_pending(deadline)
    }

    pub fn flush_and_snapshot(
        &self,
        selection: &Selection,
        retain_ids: &[i64],
    ) -> Result<SnapshotData, String> {
        let deadline = Instant::now() + FLUSH_DEADLINE;
        let newest_first = self.newest_first.load(Ordering::Relaxed);
        let _sync = self.sync_gate.lock_until(deadline)?;
        let _deadline = RemoteDeadlineGuard::new(self.remote.as_ref(), deadline);
        self.flush_pending(deadline)?;
        self.local_snapshot_until(selection, retain_ids, deadline, newest_first)
    }

    pub fn feed_icon(&self, feed_id: i64) -> crate::icons::CachedIcon {
        self.icon_service.feed_icon_with_deadline(
            feed_id,
            self.remote.as_ref(),
            Some(Instant::now() + ICON_DEADLINE),
        )
    }

    /*
     * Mutation methods below are local-only and use the public 5-second
     * deadline for store ownership and SQLite work. Their scheduled remote
     * work uses the 30-second flush deadline independently.
     */

    pub fn mark_read(
        self: &Arc<Self>,
        selection: &Selection,
        ids: &[i64],
        retain_ids: &[i64],
        read: bool,
        automatic: bool,
    ) -> Result<(SnapshotData, Option<MutationReceipt>), String> {
        let deadline = Instant::now() + LOCAL_DEADLINE;
        let newest_first = self.newest_first.load(Ordering::Relaxed);
        let (snapshot, receipt) = {
            let store = self.store.lock_until(deadline)?;
            let receipt = store
                .set_read(&self.account_id, ids, read, automatic)
                .map_err(|error| error.to_string())?;
            self.schedule_flush(if automatic {
                self.automatic_flush_delay
            } else {
                Duration::ZERO
            });
            ensure_before(deadline)?;
            let mut retained = retain_ids.to_vec();
            retained.extend_from_slice(ids);
            let snapshot =
                local_snapshot(&store, &self.account_id, newest_first, selection, &retained)?;
            ensure_before(deadline)?;
            (snapshot, receipt)
        };
        Ok((snapshot, receipt))
    }

    pub fn set_starred(
        self: &Arc<Self>,
        selection: &Selection,
        entry_id: i64,
        starred: bool,
        retain_ids: &[i64],
    ) -> Result<SnapshotData, String> {
        let deadline = Instant::now() + LOCAL_DEADLINE;
        let newest_first = self.newest_first.load(Ordering::Relaxed);
        let snapshot = {
            let store = self.store.lock_until(deadline)?;
            store
                .set_starred(&self.account_id, entry_id, starred)
                .map_err(|error| error.to_string())?;
            self.schedule_flush(Duration::ZERO);
            let mut retained = retain_ids.to_vec();
            retained.push(entry_id);
            let snapshot =
                local_snapshot(&store, &self.account_id, newest_first, selection, &retained)?;
            ensure_before(deadline)?;
            snapshot
        };
        Ok(snapshot)
    }

    pub fn undo(
        self: &Arc<Self>,
        selection: &Selection,
        receipt_id: &str,
        retain_ids: &[i64],
    ) -> Result<SnapshotData, String> {
        let deadline = Instant::now() + LOCAL_DEADLINE;
        let newest_first = self.newest_first.load(Ordering::Relaxed);
        let snapshot = {
            let store = self.store.lock_until(deadline)?;
            let ids = store
                .undo(&self.account_id, receipt_id)
                .map_err(|error| error.to_string())?;
            self.schedule_flush(Duration::ZERO);
            let mut retained = retain_ids.to_vec();
            retained.extend(ids);
            let snapshot =
                local_snapshot(&store, &self.account_id, newest_first, selection, &retained)?;
            ensure_before(deadline)?;
            snapshot
        };
        Ok(snapshot)
    }

    pub fn discard_undo(&self, receipt_id: &str) -> Result<(), String> {
        let deadline = Instant::now() + LOCAL_DEADLINE;
        self.store
            .lock_until(deadline)?
            .discard_undo(&self.account_id, receipt_id)
            .map_err(|error| error.to_string())?;
        ensure_before(deadline)
    }

    fn sync_serialized(
        &self,
        selection: &Selection,
        retain_ids: &[i64],
        deadline: Instant,
        newest_first: bool,
    ) -> Result<SyncResult, String> {
        // Go logs and ignores a pre-refresh flush error.
        let _ = self.flush_pending(deadline);
        let remote_snapshot = match browse(self.remote.as_ref(), selection) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return self
                    .local_snapshot_until(selection, retain_ids, deadline, newest_first)
                    .map(|snapshot| SyncResult::Partial(snapshot, error));
            }
        };
        self.store
            .lock_until(deadline)?
            .apply_snapshot(&self.account_id, &remote_snapshot)
            .map_err(|error| error.to_string())?;
        ensure_before(deadline)?;
        self.local_snapshot_until(selection, retain_ids, deadline, newest_first)
            .map(SyncResult::Success)
    }

    fn flush_pending(&self, deadline: Instant) -> Result<(), String> {
        let pending = self
            .store
            .lock_until(deadline)?
            .pending(&self.account_id)
            .map_err(|error| error.to_string())?;
        ensure_before(deadline)?;
        for mutation in pending {
            match mutation.field.as_str() {
                "read" => self
                    .remote
                    .set_read_batch(&[mutation.entry_id], mutation.desired)
                    .map_err(|error| format!("Miniflux-Lesestatus aktualisieren: {error}"))?,
                "starred" => {
                    let current = self
                        .remote
                        .entry_starred(mutation.entry_id)
                        .map_err(|error| format!("Miniflux-Eintrag laden: {error}"))?;
                    if current != mutation.desired {
                        self.remote
                            .toggle_starred(mutation.entry_id)
                            .map_err(|error| {
                                format!("Miniflux-Markierung aktualisieren: {error}")
                            })?;
                    }
                }
                field => return Err(format!("unbekanntes Mutationsfeld {field:?}")),
            }
            self.store
                .lock_until(deadline)?
                .acknowledge(&self.account_id, &mutation)
                .map_err(|error| error.to_string())?;
            ensure_before(deadline)?;
        }
        Ok(())
    }

    fn local_snapshot_until(
        &self,
        selection: &Selection,
        retain_ids: &[i64],
        deadline: Instant,
        newest_first: bool,
    ) -> Result<SnapshotData, String> {
        let store = self.store.lock_until(deadline)?;
        let snapshot = local_snapshot(
            &store,
            &self.account_id,
            newest_first,
            selection,
            retain_ids,
        )?;
        ensure_before(deadline)?;
        Ok(snapshot)
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
        drop(schedule);
        if std::thread::Builder::new()
            .name("fluxbar-pending-flush".to_string())
            .spawn(move || service.run_scheduler())
            .is_err()
        {
            locked(&self.scheduler.state).worker_running = false;
        }
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

fn local_snapshot(
    store: &Store,
    account_id: &str,
    newest_first: bool,
    selection: &Selection,
    retain_ids: &[i64],
) -> Result<SnapshotData, String> {
    store
        .local_snapshot(account_id, selection, newest_first, retain_ids)
        .map_err(|error| error.to_string())
}

fn ensure_before(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("context deadline exceeded".to_string())
    } else {
        Ok(())
    }
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
    struct CallGate {
        entered: (Mutex<bool>, Condvar),
        release: (Mutex<bool>, Condvar),
    }

    impl CallGate {
        fn block(&self) {
            let (entered, changed) = &self.entered;
            *locked(entered) = true;
            changed.notify_all();

            let (release, changed) = &self.release;
            let mut released = locked(release);
            while !*released {
                released = changed
                    .wait(released)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        }

        fn wait_until_entered(&self) {
            let (entered, changed) = &self.entered;
            let mut entered = locked(entered);
            while !*entered {
                let (next, timeout) = changed
                    .wait_timeout(entered, Duration::from_secs(2))
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                entered = next;
                assert!(!timeout.timed_out(), "blocked call did not enter in time");
            }
        }

        fn enters_within(&self, timeout: Duration) -> bool {
            let (entered, changed) = &self.entered;
            let entered = locked(entered);
            if *entered {
                return true;
            }
            let (entered, _) = changed
                .wait_timeout(entered, timeout)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *entered
        }

        fn release(&self) {
            let (release, changed) = &self.release;
            *locked(release) = true;
            changed.notify_all();
        }
    }

    struct ReleaseGate(Arc<CallGate>);

    impl ReleaseGate {
        fn new(gate: &Arc<CallGate>) -> Self {
            Self(Arc::clone(gate))
        }
    }

    impl Drop for ReleaseGate {
        fn drop(&mut self) {
            self.0.release();
        }
    }

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
        refresh_gate: Option<Arc<CallGate>>,
        read_gate: Option<Arc<CallGate>>,
        icon_gate: Option<Arc<CallGate>>,
        active_refreshes: usize,
        active_reads: usize,
        refresh_overlap: Option<std::sync::mpsc::Sender<()>>,
        read_overlap: Option<std::sync::mpsc::Sender<()>>,
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
            let gate = {
                let mut state = locked(&self.0);
                state.active_refreshes += 1;
                if state.active_refreshes > 1 {
                    if let Some(overlap) = &state.refresh_overlap {
                        let _ = overlap.send(());
                    }
                }
                state.refresh_gate.clone()
            };
            if let Some(gate) = gate {
                gate.block();
            }
            let mut state = locked(&self.0);
            state.active_refreshes -= 1;
            Ok(FeedCountersDto {
                unreads: state.counters.clone(),
            })
        }

        fn starred_total(&self) -> Result<i64, RemoteError> {
            Ok(locked(&self.0).starred_total)
        }

        fn icon_data_url(&self, _feed_id: i64) -> Result<Option<String>, RemoteError> {
            let gate = locked(&self.0).icon_gate.clone();
            if let Some(gate) = gate {
                gate.block();
            }
            Ok(None)
        }

        fn set_read_batch(&self, ids: &[i64], read: bool) -> Result<(), RemoteError> {
            let mut state = locked(&self.0);
            state.read_calls.push((ids[0], read));
            state.active_reads += 1;
            if state.active_reads > 1 {
                if let Some(overlap) = &state.read_overlap {
                    let _ = overlap.send(());
                }
            }
            let should_fail =
                state.read_error_at > 0 && state.read_calls.len() == state.read_error_at;
            let gate = state.read_gate.clone();
            drop(state);
            if let Some(gate) = gate {
                gate.block();
            }
            locked(&self.0).active_reads -= 1;
            if should_fail {
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
            let store = service.store.lock();
            store.set_read("account", &[1], true, false).unwrap();
            store.set_starred("account", 1, true).unwrap();
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
            let store = bootstrap.store.lock();
            store.set_read("account", &[1, 2], true, false).unwrap();
        }
        locked(&state).read_error_at = 2;
        assert!(bootstrap.flush().unwrap_err().contains("read failed"));
        let store = bootstrap.store.lock();
        let pending = store.pending("account").unwrap();
        let calls = locked(&state).read_calls.clone();
        let acknowledged_id = calls[0].0;
        let failed_id = calls[1].0;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].entry_id, failed_id);
        assert_eq!(
            store
                .entry("account", acknowledged_id)
                .unwrap()
                .unwrap()
                .remote_status
                .as_str(),
            "read"
        );
        assert_eq!(
            store
                .entry("account", failed_id)
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
        assert!(service.store.lock().pending("account").unwrap().is_empty());
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

    #[test]
    fn blocked_refresh_allows_local_snapshot_and_mutations() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();

        let gate = Arc::new(CallGate::default());
        locked(&state).refresh_gate = Some(Arc::clone(&gate));
        let release = ReleaseGate::new(&gate);
        let refreshing = Arc::clone(&service);
        let refresh = std::thread::spawn(move || refreshing.sync(&unread_selection(), &[1, 2]));
        gate.wait_until_entered();

        let (completed_tx, completed_rx) = std::sync::mpsc::channel();
        let snapshot_service = Arc::clone(&service);
        let snapshot_tx = completed_tx.clone();
        std::thread::spawn(move || {
            let progressed = snapshot_service
                .local_snapshot(&unread_selection(), &[])
                .is_ok_and(|snapshot| snapshot.version == 1);
            snapshot_tx.send(("snapshot", progressed)).unwrap();
        });
        let read_service = Arc::clone(&service);
        let read_tx = completed_tx.clone();
        std::thread::spawn(move || {
            let progressed = read_service
                .mark_read(&unread_selection(), &[1], &[], true, false)
                .is_ok_and(|(snapshot, _)| snapshot.entries[0].status.as_str() == "read");
            read_tx.send(("set_read", progressed)).unwrap();
        });
        let star_service = Arc::clone(&service);
        std::thread::spawn(move || {
            let progressed = star_service
                .set_starred(&unread_selection(), 2, true, &[])
                .is_ok_and(|snapshot| {
                    snapshot
                        .entries
                        .iter()
                        .any(|entry| entry.id == 2 && entry.starred)
                });
            completed_tx.send(("set_starred", progressed)).unwrap();
        });
        let mut completed = Vec::new();
        for _ in 0..3 {
            let (operation, progressed) = completed_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("local operation blocked behind remote refresh");
            assert!(
                progressed,
                "{operation} did not produce the expected local state"
            );
            completed.push(operation);
        }
        completed.sort_unstable();
        assert_eq!(completed, ["set_read", "set_starred", "snapshot"]);

        release.0.release();
        let refreshed = match refresh.join().unwrap().unwrap() {
            SyncResult::Success(snapshot) | SyncResult::Partial(snapshot, _) => snapshot,
        };
        assert!(
            refreshed
                .entries
                .iter()
                .any(|entry| entry.id == 1 && entry.status.as_str() == "read")
        );
        assert!(
            refreshed
                .entries
                .iter()
                .any(|entry| entry.id == 2 && entry.starred)
        );
        wait_until(|| scheduler_idle(&service));
    }

    #[test]
    fn blocked_icon_allows_local_work_and_refresh() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();

        let gate = Arc::new(CallGate::default());
        locked(&state).icon_gate = Some(Arc::clone(&gate));
        let release = ReleaseGate::new(&gate);
        let loading = Arc::clone(&service);
        let icon = std::thread::spawn(move || loading.feed_icon(20));
        gate.wait_until_entered();

        assert_eq!(
            service
                .local_snapshot(&unread_selection(), &[])
                .unwrap()
                .entries
                .len(),
            2
        );
        service
            .mark_read(&unread_selection(), &[1], &[], true, false)
            .unwrap();
        match service.sync(&unread_selection(), &[1]).unwrap() {
            SyncResult::Success(snapshot) | SyncResult::Partial(snapshot, _) => {
                assert_eq!(snapshot.entries.len(), 2);
            }
        }

        release.0.release();
        assert!(icon.join().unwrap().regular.is_empty());
        wait_until(|| scheduler_idle(&service));
    }

    #[test]
    fn blocked_flush_allows_snapshot_and_superseding_mutation() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();
        service
            .store
            .lock()
            .set_read("account", &[1], true, false)
            .unwrap();

        let gate = Arc::new(CallGate::default());
        locked(&state).read_gate = Some(Arc::clone(&gate));
        let release = ReleaseGate::new(&gate);
        let flushing = Arc::clone(&service);
        let flush = std::thread::spawn(move || flushing.flush());
        gate.wait_until_entered();

        assert_eq!(
            service
                .local_snapshot(&unread_selection(), &[1])
                .unwrap()
                .entries[0]
                .status
                .as_str(),
            "read"
        );
        service
            .mark_read(&unread_selection(), &[1], &[], false, true)
            .unwrap();

        release.0.release();
        flush.join().unwrap().unwrap();
        let store = service.store.lock();
        let pending = store.pending("account").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].entry_id, 1);
        assert!(!pending[0].desired);
        let entry = store.entry("account", 1).unwrap().unwrap();
        assert_eq!(entry.remote_status.as_str(), "read");
        assert_eq!(entry.entry.status.as_str(), "unread");
        drop(store);
        service.schedule_flush(Duration::ZERO);
        wait_until(|| service.store.lock().pending("account").unwrap().is_empty());
        wait_until(|| scheduler_idle(&service));
    }

    #[test]
    fn refresh_and_flush_remain_serialized() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();
        let refresh_gate = Arc::new(CallGate::default());
        let read_gate = Arc::new(CallGate::default());
        {
            let mut state = locked(&state);
            state.refresh_gate = Some(Arc::clone(&refresh_gate));
            state.read_gate = Some(Arc::clone(&read_gate));
        }
        let release_refresh = ReleaseGate::new(&refresh_gate);
        let release_read = ReleaseGate::new(&read_gate);
        let refreshing = Arc::clone(&service);
        let refresh = std::thread::spawn(move || refreshing.sync(&unread_selection(), &[]));
        refresh_gate.wait_until_entered();

        service
            .store
            .lock()
            .set_read("account", &[1], true, false)
            .unwrap();

        let flushing = Arc::clone(&service);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let flush = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            flushing.flush()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        assert!(
            !read_gate.enters_within(Duration::from_millis(100)),
            "flush entered remote delivery while refresh still owned synchronization"
        );
        release_refresh.0.release();
        refresh.join().unwrap().unwrap();
        read_gate.wait_until_entered();
        release_read.0.release();
        flush.join().unwrap().unwrap();
        assert_eq!(locked(&state).read_calls.len(), 1);
    }

    #[test]
    fn manual_mutation_advances_automatic_flush_without_duplication() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();
        service
            .mark_read(&unread_selection(), &[1], &[], true, true)
            .unwrap();

        let gate = Arc::new(CallGate::default());
        locked(&state).read_gate = Some(Arc::clone(&gate));
        let release = ReleaseGate::new(&gate);
        service
            .mark_read(&unread_selection(), &[2], &[], true, false)
            .unwrap();
        gate.wait_until_entered();
        release.0.release();
        wait_until(|| locked(&state).read_calls.len() == 2);
        wait_until(|| service.store.lock().pending("account").unwrap().is_empty());
        assert_eq!(locked(&state).read_calls, vec![(1, true), (2, true)]);
        wait_until(|| scheduler_idle(&service));
    }

    #[test]
    fn concurrent_refreshes_are_serialized() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        let gate = Arc::new(CallGate::default());
        let (overlap_tx, overlap_rx) = std::sync::mpsc::channel();
        {
            let mut state = locked(&state);
            state.refresh_gate = Some(Arc::clone(&gate));
            state.refresh_overlap = Some(overlap_tx);
        }
        let release = ReleaseGate::new(&gate);
        let first_service = Arc::clone(&service);
        let first = std::thread::spawn(move || first_service.sync(&unread_selection(), &[]));
        gate.wait_until_entered();
        let second_service = Arc::clone(&service);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            second_service.sync(&unread_selection(), &[])
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            overlap_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "concurrent refreshes entered remote work together"
        );
        release.0.release();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    }

    #[test]
    fn concurrent_flushes_deliver_one_pending_revision_once() {
        let (remote, state) = remote_fixture();
        let (_directory, service) = service(remote, Duration::from_secs(60));
        service.sync(&unread_selection(), &[]).unwrap();
        service
            .store
            .lock()
            .set_read("account", &[1], true, false)
            .unwrap();
        let gate = Arc::new(CallGate::default());
        let (overlap_tx, overlap_rx) = std::sync::mpsc::channel();
        {
            let mut state = locked(&state);
            state.read_gate = Some(Arc::clone(&gate));
            state.read_overlap = Some(overlap_tx);
        }
        let release = ReleaseGate::new(&gate);
        let first_service = Arc::clone(&service);
        let first = std::thread::spawn(move || first_service.flush());
        gate.wait_until_entered();
        let second_service = Arc::clone(&service);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            second_service.flush()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            overlap_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "concurrent flushes delivered the same pending revision together"
        );
        release.0.release();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
        assert_eq!(locked(&state).read_calls, vec![(1, true)]);
    }

    #[test]
    fn state_lock_wait_honors_deadline_and_recovers() {
        let state = Arc::new(StateLock::new(7));
        let held = state.lock();
        let waiting = Arc::clone(&state);
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            result_tx
                .send(
                    waiting
                        .lock_until(Instant::now() + Duration::from_millis(20))
                        .is_err(),
                )
                .unwrap();
        });
        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("state-lock waiter did not honor deadline")
        );
        drop(held);
        assert_eq!(*state.lock(), 7);
    }

    fn scheduler_idle(service: &SyncService) -> bool {
        !locked(&service.scheduler.state).worker_running
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
