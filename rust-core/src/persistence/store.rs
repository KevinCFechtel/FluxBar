use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::domain::entry::{Entry, EntryStatus};
use crate::domain::navigation::{Category, Feed};
use crate::domain::selection::Selection;

use super::schema::SCHEMA;

/// Presentation cap identical to Go's `snapshotLimit`.
pub const SNAPSHOT_LIMIT: i64 = 200;

/// Fully assembled local snapshot in domain-level representations.
///
/// Serialization happens in the transport layer; this type intentionally
/// carries no serde attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotData {
    pub version: i32,
    pub selection: Selection,
    pub entries: Vec<Entry>,
    pub categories: Vec<Category>,
    pub total: i32,
    pub unread_total: i32,
    pub starred_total: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationReceipt {
    pub id: String,
    pub count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMutation {
    pub entry_id: i64,
    pub field: String,
    pub desired: bool,
    pub revision: i64,
}

/// Maps a normalized domain selection onto Go's SQL `selectionClause`.
fn selection_clause(selection: &Selection) -> (String, Vec<Value>) {
    let mut parts = vec!["account_id=?1".to_string()];
    let mut arguments: Vec<Value> = Vec::new();

    if selection.is_unread_only() {
        parts.push("status='unread'".to_string());
    }
    match selection {
        Selection::Starred { .. } => parts.push("starred=1".to_string()),
        Selection::Category { id, .. } => {
            parts.push(format!("category_id=?{}", arguments.len() + 2));
            arguments.push(Value::from(*id));
        }
        Selection::Feed { id, .. } => {
            parts.push(format!("feed_id=?{}", arguments.len() + 2));
            arguments.push(Value::from(*id));
        }
        _ => {}
    }
    (parts.join(" AND "), arguments)
}

/// Normalization shared with the store entry point (mirrors
/// `Selection.Normalized`); delegates to the domain implementation.
fn normalize_selection(selection: &Selection) -> Selection {
    match selection {
        Selection::All { .. } | Selection::Unread { .. } | Selection::Starred { .. } => {
            selection.clone()
        }
        Selection::Category { id, unread_only } if *id > 0 => selection.clone(),
        Selection::Feed { id, unread_only } if *id > 0 => selection.clone(),
        _ => Selection::All {
            id: 0,
            unread_only: true,
        },
    }
}

fn selection_kind(selection: &Selection) -> &'static str {
    match selection {
        Selection::All { .. } => "all",
        Selection::Unread { .. } => "unread",
        Selection::Starred { .. } => "starred",
        Selection::Category { .. } => "category",
        Selection::Feed { .. } => "feed",
    }
}

fn map_entry_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let status: String = row.get(10)?;
    Ok(Entry {
        id: row.get(0)?,
        title: row.get(1)?,
        url: row.get(2)?,
        comments_url: row.get(3)?,
        feed_id: row.get(4)?,
        feed_name: row.get(5)?,
        category_id: row.get(6)?,
        published_at_rfc3339: row.get(7)?,
        preview: row.get(8)?,
        image_url: row.get(9)?,
        status: EntryStatus::parse(&status),
        starred: row.get(11)?,
    })
}

#[derive(Debug)]
pub enum OpenError {
    Sqlite(rusqlite::Error),
    Filesystem(std::io::Error),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Sqlite(error) => write!(formatter, "SQLite store: {error}"),
            OpenError::Filesystem(error) => write!(formatter, "database permissions: {error}"),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<rusqlite::Error> for OpenError {
    fn from(error: rusqlite::Error) -> Self {
        OpenError::Sqlite(error)
    }
}

/// Entry domain data plus remote baseline state retained only by persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedEntry {
    pub entry: Entry,
    pub remote_status: EntryStatus,
    pub remote_starred: bool,
}

/// A single-connection SQLite store compatible with the Go implementation.
///
/// `Connection` is held directly rather than pooled, matching Go's
/// `SetMaxOpenConns(1)` concurrency assumption and keeping PRAGMAs and temp
/// tables connection-local.
pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens or creates a store at an explicitly supplied path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, OpenError> {
        let path = path.as_ref();
        log::debug!(target: "persistence", "store open started");
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(5_000))?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;",
        )?;
        connection.execute_batch(SCHEMA)?;
        set_private_file_permissions(path).map_err(OpenError::Filesystem)?;
        log::info!(target: "persistence", "store opened");
        Ok(Self { connection })
    }

    /// Creates or updates an account without resetting counters/timestamps.
    pub fn ensure_account(&self, account_id: &str, server: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO accounts(id, server) VALUES(?1, ?2)
             ON CONFLICT(id) DO UPDATE SET server=excluded.server",
            params![account_id, server],
        )?;
        Ok(())
    }

    /// Low-level account-scoped category upsert used by compatibility probes.
    pub fn upsert_category(&self, account_id: &str, category: &Category) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO categories(account_id,id,title) VALUES(?1,?2,?3)
             ON CONFLICT(account_id,id) DO UPDATE SET title=excluded.title",
            params![account_id, category.id, category.title],
        )?;
        Ok(())
    }

    /// Low-level account-scoped feed upsert.
    pub fn upsert_feed(&self, account_id: &str, feed: &Feed) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(account_id,id) DO UPDATE SET
             category_id=excluded.category_id,title=excluded.title,
             remote_unread_count=excluded.remote_unread_count",
            params![
                account_id,
                feed.id,
                feed.category_id,
                feed.title,
                feed.unread_count
            ],
        )?;
        Ok(())
    }

    /// Stores a normalized selection total using SQLite integer Booleans.
    pub fn upsert_selection_total(
        &self,
        account_id: &str,
        kind: &str,
        selection_id: i64,
        unread_only: bool,
        total: i32,
    ) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO selection_totals(account_id,kind,selection_id,unread_only,total)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(account_id,kind,selection_id,unread_only)
             DO UPDATE SET total=excluded.total",
            params![account_id, kind, selection_id, unread_only, total],
        )?;
        Ok(())
    }

    /// Writes an exact entry persistence record without applying sync or
    /// pending-mutation reconciliation semantics.
    pub fn upsert_entry(&self, account_id: &str, record: &PersistedEntry) -> rusqlite::Result<()> {
        let entry = &record.entry;
        self.connection.execute(
            "INSERT INTO entries(
             account_id,id,title,url,comments_url,feed_id,feed_name,category_id,
             published_at,preview,image_url,remote_status,remote_starred,status,starred)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
             ON CONFLICT(account_id,id) DO UPDATE SET
             title=excluded.title,url=excluded.url,comments_url=excluded.comments_url,
             feed_id=excluded.feed_id,feed_name=excluded.feed_name,
             category_id=excluded.category_id,published_at=excluded.published_at,
             preview=excluded.preview,image_url=excluded.image_url,
             remote_status=excluded.remote_status,remote_starred=excluded.remote_starred,
             status=excluded.status,starred=excluded.starred",
            params![
                account_id,
                entry.id,
                entry.title,
                entry.url,
                entry.comments_url,
                entry.feed_id,
                entry.feed_name,
                entry.category_id,
                entry.published_at_rfc3339,
                entry.preview,
                entry.image_url,
                record.remote_status.as_str(),
                record.remote_starred,
                entry.status.as_str(),
                entry.starred,
            ],
        )?;
        Ok(())
    }

    /// Reads one account-scoped entry, preserving unknown status strings.
    pub fn entry(
        &self,
        account_id: &str,
        entry_id: i64,
    ) -> rusqlite::Result<Option<PersistedEntry>> {
        self.connection
            .query_row(
                "SELECT id,title,url,comments_url,feed_id,feed_name,category_id,
                 published_at,preview,image_url,remote_status,remote_starred,status,starred
                 FROM entries WHERE account_id=?1 AND id=?2",
                params![account_id, entry_id],
                |row| {
                    let remote_status: String = row.get(10)?;
                    let status: String = row.get(12)?;
                    Ok(PersistedEntry {
                        entry: Entry {
                            id: row.get(0)?,
                            title: row.get(1)?,
                            url: row.get(2)?,
                            comments_url: row.get(3)?,
                            feed_id: row.get(4)?,
                            feed_name: row.get(5)?,
                            category_id: row.get(6)?,
                            published_at_rfc3339: row.get(7)?,
                            preview: row.get(8)?,
                            image_url: row.get(9)?,
                            status: EntryStatus::parse(&status),
                            starred: row.get(13)?,
                        },
                        remote_status: EntryStatus::parse(&remote_status),
                        remote_starred: row.get(11)?,
                    })
                },
            )
            .optional()
    }

    /// Applies one fully assembled remote browse result atomically. Effective
    /// local read/star values survive when their corresponding pending row
    /// exists; remote baselines always advance to the observed remote value.
    pub fn apply_snapshot(
        &self,
        account_id: &str,
        snapshot: &SnapshotData,
    ) -> rusqlite::Result<()> {
        let start = std::time::Instant::now();
        log::info!(
            target: "reconciliation",
            "apply_snapshot started entries={} categories={} feeds={} total={} starred_total={}",
            snapshot.entries.len(),
            snapshot.categories.len(),
            snapshot.categories.iter().map(|c| c.feeds.len()).sum::<usize>(),
            snapshot.total,
            snapshot.starred_total
        );
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE accounts SET remote_starred_total=?1,last_sync_at=?2 WHERE id=?3",
            params![snapshot.starred_total, now_rfc3339(), account_id],
        )?;

        let selection = normalize_selection(&snapshot.selection);
        transaction.execute(
            "INSERT INTO selection_totals(account_id,kind,selection_id,unread_only,total)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(account_id,kind,selection_id,unread_only)
             DO UPDATE SET total=excluded.total",
            params![
                account_id,
                selection_kind(&selection),
                selection.scope_id(),
                selection.is_unread_only(),
                snapshot.total
            ],
        )?;

        if snapshot.total <= snapshot.entries.len() as i32
            && (selection.is_unread_only() || matches!(selection, Selection::Starred { .. }))
        {
            reconcile_complete_selection(&transaction, account_id, &selection, &snapshot.entries)?;
        }

        transaction.execute("DELETE FROM feeds WHERE account_id=?1", [account_id])?;
        transaction.execute("DELETE FROM categories WHERE account_id=?1", [account_id])?;
        for category in &snapshot.categories {
            transaction.execute(
                "INSERT INTO categories(account_id,id,title) VALUES(?1,?2,?3)",
                params![account_id, category.id, category.title],
            )?;
            for feed in &category.feeds {
                transaction.execute(
                    "INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count)
                     VALUES(?1,?2,?3,?4,?5)",
                    params![
                        account_id,
                        feed.id,
                        category.id,
                        feed.title,
                        feed.unread_count
                    ],
                )?;
            }
        }

        for entry in &snapshot.entries {
            let pending_read: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM pending_mutations
                 WHERE account_id=?1 AND entry_id=?2 AND field='read'",
                params![account_id, entry.id],
                |row| row.get(0),
            )?;
            let pending_starred: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM pending_mutations
                 WHERE account_id=?1 AND entry_id=?2 AND field='starred'",
                params![account_id, entry.id],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO entries(
                 account_id,id,title,url,comments_url,feed_id,feed_name,category_id,published_at,
                 preview,image_url,remote_status,remote_starred,status,starred)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
                 ON CONFLICT(account_id,id) DO UPDATE SET
                 title=excluded.title,url=excluded.url,comments_url=excluded.comments_url,
                 feed_id=excluded.feed_id,feed_name=excluded.feed_name,
                 category_id=excluded.category_id,published_at=excluded.published_at,
                 preview=excluded.preview,image_url=excluded.image_url,
                 remote_status=excluded.remote_status,remote_starred=excluded.remote_starred,
                 status=CASE WHEN ?16 THEN entries.status ELSE excluded.status END,
                 starred=CASE WHEN ?17 THEN entries.starred ELSE excluded.starred END",
                params![
                    account_id,
                    entry.id,
                    entry.title,
                    entry.url,
                    entry.comments_url,
                    entry.feed_id,
                    entry.feed_name,
                    entry.category_id,
                    entry.published_at_rfc3339,
                    entry.preview,
                    entry.image_url,
                    entry.status.as_str(),
                    entry.starred,
                    entry.status.as_str(),
                    entry.starred,
                    pending_read > 0,
                    pending_starred > 0,
                ],
            )?;
        }
        match transaction.commit() {
            Ok(()) => {
                let elapsed = start.elapsed().as_millis();
                log::info!(
                    target: "reconciliation",
                    "apply_snapshot completed duration_ms={elapsed}"
                );
                Ok(())
            }
            Err(error) => {
                log::error!(
                    target: "reconciliation",
                    "apply_snapshot failed category={} error={}",
                    sqlite_error_category(&error),
                    sqlite_error_summary(&error)
                );
                Err(error)
            }
        }
    }

    pub fn set_read(
        &self,
        account_id: &str,
        entry_ids: &[i64],
        read: bool,
        undoable: bool,
    ) -> rusqlite::Result<Option<MutationReceipt>> {
        let start = std::time::Instant::now();
        let transaction = self.connection.unchecked_transaction()?;
        let mut receipt = MutationReceipt {
            id: random_id(),
            count: 0,
        };
        if undoable {
            transaction.execute(
                "INSERT INTO undo_batches(account_id,id,created_at) VALUES(?1,?2,?3)",
                params![account_id, receipt.id, now_rfc3339()],
            )?;
        }
        let status = if read { "read" } else { "unread" };
        for entry_id in entry_ids {
            if !undoable {
                transaction.execute(
                    "DELETE FROM undo_items WHERE account_id=?1 AND entry_id=?2",
                    params![account_id, entry_id],
                )?;
            }
            let current: Option<String> = transaction
                .query_row(
                    "SELECT status FROM entries WHERE account_id=?1 AND id=?2",
                    params![account_id, entry_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(current) = current else { continue };
            if current == status {
                continue;
            }
            if undoable {
                transaction.execute(
                    "INSERT INTO undo_items(account_id,batch_id,entry_id,prior_read)
                     VALUES(?1,?2,?3,?4)",
                    params![account_id, receipt.id, entry_id, current == "read"],
                )?;
            }
            transaction.execute(
                "UPDATE entries SET status=?1 WHERE account_id=?2 AND id=?3",
                params![status, account_id, entry_id],
            )?;
            upsert_pending(&transaction, account_id, *entry_id, "read", read)?;
            receipt.count += 1;
        }
        if undoable && receipt.count == 0 {
            transaction.execute(
                "DELETE FROM undo_batches WHERE account_id=?1 AND id=?2",
                params![account_id, receipt.id],
            )?;
        }
        if !undoable {
            transaction.execute(
                "DELETE FROM undo_batches WHERE account_id=?1 AND NOT EXISTS(
                 SELECT 1 FROM undo_items i WHERE i.account_id=undo_batches.account_id
                 AND i.batch_id=undo_batches.id)",
                [account_id],
            )?;
        }
        transaction.commit()?;
        let elapsed = start.elapsed().as_millis();
        log::info!(
            target: "persistence",
            "mutation persistence completed operation=set_read count={} undoable={undoable} duration_ms={elapsed}",
            receipt.count
        );
        Ok((undoable && receipt.count > 0).then_some(receipt))
    }

    pub fn set_starred(
        &self,
        account_id: &str,
        entry_id: i64,
        starred: bool,
    ) -> rusqlite::Result<()> {
        let start = std::time::Instant::now();
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE entries SET starred=?1 WHERE account_id=?2 AND id=?3",
            params![starred, account_id, entry_id],
        )?;
        upsert_pending(&transaction, account_id, entry_id, "starred", starred)?;
        transaction.commit()?;
        let elapsed = start.elapsed().as_millis();
        log::info!(
            target: "persistence",
            "mutation persistence completed operation=set_starred entry_id={entry_id} duration_ms={elapsed}"
        );
        Ok(())
    }

    pub fn pending(&self, account_id: &str) -> rusqlite::Result<Vec<PendingMutation>> {
        let start = std::time::Instant::now();
        let mut statement = self.connection.prepare(
            "SELECT entry_id,field,desired,revision FROM pending_mutations
             WHERE account_id=?1 ORDER BY updated_at",
        )?;
        let result: rusqlite::Result<Vec<_>> = statement
            .query_map([account_id], |row| {
                Ok(PendingMutation {
                    entry_id: row.get(0)?,
                    field: row.get(1)?,
                    desired: row.get(2)?,
                    revision: row.get(3)?,
                })
            })?
            .collect();
        if let Ok(ref items) = result {
            let elapsed = start.elapsed().as_millis();
            log::debug!(
                target: "persistence",
                "pending query completed count={} duration_ms={elapsed}",
                items.len()
            );
        }
        result
    }

    /// Lightweight COUNT of pending mutations for diagnostics. Bounded by the
    /// number of pending rows, so it is safe for startup logging.
    pub fn pending_count(&self, account_id: &str) -> rusqlite::Result<i64> {
        self.connection.query_row(
            "SELECT COUNT(*) FROM pending_mutations WHERE account_id=?1",
            [account_id],
            |row| row.get(0),
        )
    }

    /// Lightweight COUNT of undo batches for diagnostics.
    pub fn undo_count(&self, account_id: &str) -> rusqlite::Result<i64> {
        self.connection.query_row(
            "SELECT COUNT(*) FROM undo_batches WHERE account_id=?1",
            [account_id],
            |row| row.get(0),
        )
    }

    pub fn acknowledge(
        &self,
        account_id: &str,
        mutation: &PendingMutation,
    ) -> rusqlite::Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        match mutation.field.as_str() {
            "read" => {
                let (previous, feed_id, category_id): (String, i64, i64) = transaction.query_row(
                    "SELECT remote_status,feed_id,category_id FROM entries
                     WHERE account_id=?1 AND id=?2",
                    params![account_id, mutation.entry_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                let delta = match (previous.as_str(), mutation.desired) {
                    ("unread", true) => -1,
                    ("read", false) => 1,
                    _ => 0,
                };
                if delta != 0 {
                    transaction.execute(
                        "UPDATE feeds SET remote_unread_count=MAX(0,remote_unread_count+?1)
                         WHERE account_id=?2 AND id=?3",
                        params![delta, account_id, feed_id],
                    )?;
                    transaction.execute(
                        "UPDATE selection_totals SET total=MAX(0,total+?1)
                         WHERE account_id=?2 AND (kind='unread' OR
                         (unread_only=1 AND (kind='all' OR
                         (kind='feed' AND selection_id=?3) OR
                         (kind='category' AND selection_id=?4))))",
                        params![delta, account_id, feed_id, category_id],
                    )?;
                }
                let value = if mutation.desired { "read" } else { "unread" };
                transaction.execute(
                    "UPDATE entries SET remote_status=?1 WHERE account_id=?2 AND id=?3",
                    params![value, account_id, mutation.entry_id],
                )?;
            }
            _ => {
                let previous: bool = transaction.query_row(
                    "SELECT remote_starred FROM entries WHERE account_id=?1 AND id=?2",
                    params![account_id, mutation.entry_id],
                    |row| row.get(0),
                )?;
                let delta = match (previous, mutation.desired) {
                    (false, true) => 1,
                    (true, false) => -1,
                    _ => 0,
                };
                if delta != 0 {
                    transaction.execute(
                        "UPDATE accounts SET remote_starred_total=MAX(0,remote_starred_total+?1)
                         WHERE id=?2",
                        params![delta, account_id],
                    )?;
                    transaction.execute(
                        "UPDATE selection_totals SET total=MAX(0,total+?1)
                         WHERE account_id=?2 AND kind='starred'",
                        params![delta, account_id],
                    )?;
                }
                transaction.execute(
                    "UPDATE entries SET remote_starred=?1 WHERE account_id=?2 AND id=?3",
                    params![mutation.desired, account_id, mutation.entry_id],
                )?;
            }
        }
        transaction.execute(
            "DELETE FROM pending_mutations WHERE account_id=?1 AND entry_id=?2
             AND field=?3 AND revision=?4",
            params![
                account_id,
                mutation.entry_id,
                mutation.field,
                mutation.revision
            ],
        )?;
        transaction.commit()
    }

    pub fn undo(&self, account_id: &str, batch_id: &str) -> rusqlite::Result<Vec<i64>> {
        let transaction = self.connection.unchecked_transaction()?;
        let items: Vec<(i64, bool)> = {
            let mut statement = transaction.prepare(
                "SELECT entry_id,prior_read FROM undo_items WHERE account_id=?1 AND batch_id=?2",
            )?;
            statement
                .query_map(params![account_id, batch_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<rusqlite::Result<_>>()?
        };
        for (entry_id, prior_read) in &items {
            let status = if *prior_read { "read" } else { "unread" };
            transaction.execute(
                "UPDATE entries SET status=?1 WHERE account_id=?2 AND id=?3",
                params![status, account_id, entry_id],
            )?;
            upsert_pending(&transaction, account_id, *entry_id, "read", *prior_read)?;
        }
        transaction.execute(
            "DELETE FROM undo_items WHERE account_id=?1 AND batch_id=?2",
            params![account_id, batch_id],
        )?;
        transaction.execute(
            "DELETE FROM undo_batches WHERE account_id=?1 AND id=?2",
            params![account_id, batch_id],
        )?;
        transaction.commit()?;
        Ok(items.into_iter().map(|item| item.0).collect())
    }

    pub fn discard_undo(&self, account_id: &str, batch_id: &str) -> rusqlite::Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM undo_items WHERE account_id=?1 AND batch_id=?2",
            params![account_id, batch_id],
        )?;
        transaction.execute(
            "DELETE FROM undo_batches WHERE account_id=?1 AND id=?2",
            params![account_id, batch_id],
        )?;
        transaction.commit()
    }

    /// Assembles the complete local snapshot exactly like
    /// `go-core/internal/inbox/store.go Snapshot`.
    ///
    /// The caller supplies an account ID and a selection; normalization is
    /// applied here (mirroring Go). Retained IDs are ORed into the entry
    /// query before the 200-row limit, reproducing presentation retention.
    pub fn local_snapshot(
        &self,
        account_id: &str,
        selection: &Selection,
        newest_first: bool,
        retain_ids: &[i64],
    ) -> rusqlite::Result<SnapshotData> {
        let start = std::time::Instant::now();
        let normalized = normalize_selection(selection);
        let kind = selection_kind(&normalized);
        log::debug!(
            target: "persistence",
            "snapshot query started selection={kind} id={} unread_only={}",
            normalized.scope_id(),
            normalized.is_unread_only()
        );
        let (categories, unread_total) = self.navigation(account_id)?;
        let starred_total = self.starred_total(account_id)?;

        let (where_clause, clause_arguments) = selection_clause(&normalized);
        let mut arguments = vec![Value::from(account_id.to_string())];
        arguments.extend(clause_arguments);

        let total: i32 = self.connection.query_row(
            &format!("SELECT COUNT(*) FROM entries WHERE {where_clause}"),
            params_from_iter(arguments.iter()),
            |row| row.get(0),
        )?;

        let total = match self.selection_total(
            account_id,
            selection_kind(&normalized),
            normalized.scope_id(),
            normalized.is_unread_only(),
        )? {
            Some(remote_total) => {
                let delta = self.selection_pending_delta(account_id, &normalized)?;
                std::cmp::max(total, std::cmp::max(0, remote_total + delta))
            }
            None => total,
        };

        let entries = self.snapshot_entries(
            account_id,
            &where_clause,
            &arguments,
            newest_first,
            retain_ids,
        )?;

        let elapsed = start.elapsed().as_millis();
        log::info!(
            target: "persistence",
            "snapshot query completed rows={} selection={kind} id={} duration_ms={elapsed}",
            entries.len(),
            normalized.scope_id()
        );

        Ok(SnapshotData {
            version: 1,
            selection: normalized,
            entries,
            categories,
            total,
            unread_total,
            starred_total,
        })
    }

    /// Categories with feeds and pending-adjusted unread counts.
    fn navigation(&self, account_id: &str) -> rusqlite::Result<(Vec<Category>, i32)> {
        let mut statement = self.connection.prepare(
            "SELECT id,title FROM categories WHERE account_id=?1 ORDER BY title COLLATE NOCASE",
        )?;
        let mut categories: Vec<Category> = statement
            .query_map([account_id], |row| {
                Ok(Category {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    unread_count: 0,
                    feeds: Vec::new(),
                })
            })?
            .collect::<Result<_, _>>()?;

        let mut unread_total: i32 = 0;
        for category in &mut categories {
            let mut feed_statement = self.connection.prepare(
                "SELECT id,title,remote_unread_count FROM feeds WHERE account_id=?1 AND category_id=?2 ORDER BY title COLLATE NOCASE",
            )?;
            let feeds: Vec<(i64, String, i32)> = feed_statement
                .query_map(params![account_id, category.id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<Result<_, _>>()?;

            for (id, title, remote_count) in feeds {
                // Go ignores errors from this query (`_ =`); default to 0.
                let delta = self
                    .pending_read_delta_for_feed(account_id, id)
                    .unwrap_or(0);
                let unread_count = std::cmp::max(0, remote_count + delta);
                category.unread_count += unread_count;
                unread_total += unread_count;
                category.feeds.push(Feed {
                    id,
                    title,
                    category_id: category.id,
                    unread_count,
                });
            }
        }
        Ok((categories, unread_total))
    }

    fn pending_read_delta_for_feed(&self, account_id: &str, feed_id: i64) -> rusqlite::Result<i32> {
        self.connection.query_row(
            "SELECT COALESCE(SUM(CASE WHEN e.status='unread' AND e.remote_status='read' THEN 1 WHEN e.status='read' AND e.remote_status='unread' THEN -1 ELSE 0 END),0)
             FROM entries e JOIN pending_mutations p ON p.account_id=e.account_id AND p.entry_id=e.id AND p.field='read'
             WHERE e.account_id=?1 AND e.feed_id=?2",
            params![account_id, feed_id],
            |row| row.get(0),
        )
    }

    fn starred_total(&self, account_id: &str) -> rusqlite::Result<i32> {
        let remote_total: i64 = self.connection.query_row(
            "SELECT remote_starred_total FROM accounts WHERE id=?1",
            params![account_id],
            |row| row.get(0),
        )?;
        let delta: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(CASE WHEN e.starred=1 AND e.remote_starred=0 THEN 1 WHEN e.starred=0 AND e.remote_starred=1 THEN -1 ELSE 0 END),0)
             FROM entries e JOIN pending_mutations p ON p.account_id=e.account_id AND p.entry_id=e.id AND p.field='starred' WHERE e.account_id=?1",
            params![account_id],
            |row| row.get(0),
        )?;
        Ok(std::cmp::max(0, (remote_total + delta) as i64) as i32)
    }

    fn selection_total(
        &self,
        account_id: &str,
        kind: &str,
        selection_id: i64,
        unread_only: bool,
    ) -> rusqlite::Result<Option<i32>> {
        self.connection
            .query_row(
                "SELECT total FROM selection_totals WHERE account_id=?1 AND kind=?2 AND selection_id=?3 AND unread_only=?4",
                params![account_id, kind, selection_id, unread_only],
                |row| row.get(0),
            )
            .optional()
    }

    fn selection_pending_delta(
        &self,
        account_id: &str,
        selection: &Selection,
    ) -> rusqlite::Result<i32> {
        let read_side = matches!(
            selection,
            Selection::Unread { .. }
                | Selection::All { .. }
                | Selection::Category { .. }
                | Selection::Feed { .. }
        ) && selection.is_unread_only();
        let starred_side = matches!(selection, Selection::Starred { .. });

        if !read_side && !starred_side {
            return Ok(0);
        }

        let field = if read_side { "read" } else { "starred" };
        let expression = if read_side {
            "CASE WHEN e.status='unread' AND e.remote_status='read' THEN 1 WHEN e.status='read' AND e.remote_status='unread' THEN -1 ELSE 0 END"
        } else {
            "CASE WHEN e.starred=1 AND e.remote_starred=0 THEN 1 WHEN e.starred=0 AND e.remote_starred=1 THEN -1 ELSE 0 END"
        };

        let mut sql = format!(
            "SELECT COALESCE(SUM({expression}),0) FROM entries e JOIN pending_mutations p
             ON p.account_id=e.account_id AND p.entry_id=e.id AND p.field=?1 WHERE e.account_id=?2"
        );
        let mut parameters = vec![
            Value::from(field.to_string()),
            Value::from(account_id.to_string()),
        ];
        match selection {
            Selection::Category { id, .. } => {
                sql.push_str(" AND e.category_id=?3");
                parameters.push(Value::from(*id));
            }
            Selection::Feed { id, .. } => {
                sql.push_str(" AND e.feed_id=?3");
                parameters.push(Value::from(*id));
            }
            _ => {}
        }

        self.connection
            .query_row(&sql, params_from_iter(parameters.iter()), |row| row.get(0))
    }

    fn snapshot_entries(
        &self,
        account_id: &str,
        where_clause: &str,
        arguments: &[Value],
        newest_first: bool,
        retain_ids: &[i64],
    ) -> rusqlite::Result<Vec<Entry>> {
        let direction = if newest_first { "DESC" } else { "ASC" };
        let mut sql = format!(
            "SELECT id,title,url,comments_url,feed_id,feed_name,category_id,published_at,preview,image_url,status,starred
             FROM entries WHERE ({where_clause})"
        );
        let mut parameters: Vec<Value> = arguments.to_vec();

        if !retain_ids.is_empty() {
            let placeholders: Vec<String> = (0..retain_ids.len())
                .map(|index| format!("?{}", parameters.len() + 2 + index))
                .collect();
            sql.push_str(&format!(
                " OR (account_id=?{} AND id IN ({}))",
                parameters.len() + 1,
                placeholders.join(",")
            ));
            parameters.push(Value::from(account_id.to_string()));
            parameters.extend(retain_ids.iter().copied().map(Value::from));
        }

        sql.push_str(&format!(
            " ORDER BY published_at {direction} LIMIT {}",
            SNAPSHOT_LIMIT
        ));

        let mut statement = self.connection.prepare(&sql)?;
        let mut seen = std::collections::HashSet::new();
        let rows = statement.query_map(params_from_iter(parameters.iter()), map_entry_row)?;
        let mut entries = Vec::new();
        for row in rows {
            let entry = row?;
            if seen.insert(entry.id) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    #[cfg(test)]
    pub(super) fn connection(&self) -> &Connection {
        &self.connection
    }
}

fn reconcile_complete_selection(
    transaction: &rusqlite::Transaction<'_>,
    account_id: &str,
    selection: &Selection,
    entries: &[Entry],
) -> rusqlite::Result<()> {
    transaction.execute(
        "CREATE TEMP TABLE IF NOT EXISTS fluxbar_remote_selection_ids(id INTEGER PRIMARY KEY)",
        [],
    )?;
    transaction.execute("DELETE FROM fluxbar_remote_selection_ids", [])?;
    {
        let mut insert =
            transaction.prepare("INSERT INTO fluxbar_remote_selection_ids(id) VALUES(?1)")?;
        for entry in entries {
            insert.execute([entry.id])?;
        }
    }
    let mut parts = vec!["account_id=?1".to_string()];
    let mut arguments = vec![Value::from(account_id.to_string())];
    match selection {
        Selection::Category { id, .. } => {
            parts.push("category_id=?2".to_string());
            arguments.push(Value::from(*id));
        }
        Selection::Feed { id, .. } => {
            parts.push("feed_id=?2".to_string());
            arguments.push(Value::from(*id));
        }
        _ => {}
    }
    parts.push(
        "NOT EXISTS (SELECT 1 FROM fluxbar_remote_selection_ids remote WHERE remote.id=entries.id)"
            .to_string(),
    );
    let where_clause = parts.join(" AND ");
    if selection.is_unread_only() {
        transaction.execute(
            &format!(
                "UPDATE entries SET remote_status='read',status=CASE WHEN EXISTS(
                 SELECT 1 FROM pending_mutations p WHERE p.account_id=entries.account_id
                 AND p.entry_id=entries.id AND p.field='read') THEN status ELSE 'read' END
                 WHERE {where_clause} AND remote_status='unread'"
            ),
            params_from_iter(arguments.iter()),
        )?;
    } else if matches!(selection, Selection::Starred { .. }) {
        transaction.execute(
            &format!(
                "UPDATE entries SET remote_starred=0,starred=CASE WHEN EXISTS(
                 SELECT 1 FROM pending_mutations p WHERE p.account_id=entries.account_id
                 AND p.entry_id=entries.id AND p.field='starred') THEN starred ELSE 0 END
                 WHERE {where_clause} AND remote_starred=1"
            ),
            params_from_iter(arguments.iter()),
        )?;
    }
    Ok(())
}

fn upsert_pending(
    transaction: &rusqlite::Transaction<'_>,
    account_id: &str,
    entry_id: i64,
    field: &str,
    desired: bool,
) -> rusqlite::Result<()> {
    transaction.execute(
        "INSERT INTO pending_mutations(account_id,entry_id,field,desired,revision,updated_at)
         VALUES(?1,?2,?3,?4,1,?5)
         ON CONFLICT(account_id,entry_id,field) DO UPDATE SET
         desired=excluded.desired,revision=pending_mutations.revision+1,updated_at=excluded.updated_at",
        params![account_id, entry_id, field, desired, now_rfc3339()],
    )?;
    Ok(())
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC timestamp formatting cannot fail")
}

fn random_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed) as u128;
    format!("{:032x}", nanos ^ sequence)
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Returns a stable, content-free category for a SQLite error. The category
/// is safe for Release logs; no SQL text, bind values, or file paths are
/// included.
pub fn sqlite_error_category(error: &rusqlite::Error) -> &'static str {
    use rusqlite::Error;
    match error {
        Error::SqliteFailure(code, _) => match code.code {
            rusqlite::ErrorCode::DatabaseBusy => "busy",
            rusqlite::ErrorCode::DatabaseLocked => "locked",
            rusqlite::ErrorCode::ConstraintViolation => "constraint",
            rusqlite::ErrorCode::OperationInterrupted => "interrupted",
            _ => "sqlite",
        },
        Error::InvalidPath(_) | Error::SqliteSingleThreadedMode | Error::InvalidQuery => {
            "configuration"
        }
        Error::FromSqlConversionFailure(_, _, _) | Error::IntegralValueOutOfRange(_, _) => {
            "conversion"
        }
        Error::InvalidColumnIndex(_) | Error::InvalidColumnName(_) => "schema",
        _ => "other",
    }
}

/// A short, sanitized summary of a SQLite error suitable for logging. This
/// deliberately omits SQL statements, paths, and bind parameters.
pub fn sqlite_error_summary(error: &rusqlite::Error) -> String {
    use rusqlite::Error;
    match error {
        Error::SqliteFailure(code, _) => format!("{:?}", code.code),
        Error::InvalidPath(_) => "invalid database path".to_string(),
        Error::FromSqlConversionFailure(_, _, _) => "column conversion failed".to_string(),
        Error::IntegralValueOutOfRange(_, _) => "integer out of range".to_string(),
        Error::InvalidColumnIndex(_) => "invalid column index".to_string(),
        Error::InvalidColumnName(name) => format!("invalid column name: {name}"),
        Error::InvalidQuery => "invalid query".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn sqlite_error_category_classifies_common_errors() {
        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 0,
            },
            None,
        );
        assert_eq!(sqlite_error_category(&busy), "busy");

        let locked = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseLocked,
                extended_code: 0,
            },
            None,
        );
        assert_eq!(sqlite_error_category(&locked), "locked");

        let constraint = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                extended_code: 0,
            },
            None,
        );
        assert_eq!(sqlite_error_category(&constraint), "constraint");
    }

    #[test]
    fn sqlite_error_summary_omits_paths_and_sql() {
        let invalid_path = rusqlite::Error::InvalidPath(std::path::PathBuf::from(
            "/Users/example/secret/db.sqlite",
        ));
        let summary = sqlite_error_summary(&invalid_path);
        assert!(!summary.contains("secret"));
        assert!(!summary.contains(".sqlite"));
        assert_eq!(summary, "invalid database path");
    }

    fn open_temp_store() -> (TestDirectory, std::path::PathBuf, Store) {
        let directory = TestDirectory::new();
        let path = directory.path().join("inbox.sqlite3");
        let store = Store::open(&path).unwrap();
        (directory, path, store)
    }

    fn sample_entry(id: i64, status: EntryStatus) -> PersistedEntry {
        PersistedEntry {
            entry: Entry {
                id,
                title: format!("Entry {id}"),
                url: format!("https://example.com/{id}"),
                comments_url: String::new(),
                feed_id: 20,
                feed_name: "Feed".to_string(),
                category_id: 10,
                published_at_rfc3339: "2026-08-20T10:00:00.123456789Z".to_string(),
                preview: "Preview".to_string(),
                image_url: String::new(),
                status,
                starred: true,
            },
            remote_status: EntryStatus::Unread,
            remote_starred: false,
        }
    }

    #[test]
    fn creates_go_compatible_schema_and_pragmas() {
        let (_directory, _path, store) = open_temp_store();
        let connection = store.connection();

        let tables: HashSet<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for expected in [
            "accounts",
            "categories",
            "feeds",
            "selection_totals",
            "entries",
            "pending_mutations",
            "undo_batches",
            "undo_items",
        ] {
            assert!(tables.contains(expected), "missing table {expected}");
        }

        let indexes: HashSet<String> = connection
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_autoindex_%'",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            indexes,
            HashSet::from([
                "entries_account_published".to_string(),
                "entries_account_feed".to_string(),
                "entries_account_category".to_string(),
            ])
        );

        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        assert_eq!(
            connection
                .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1,
            "SQLite NORMAL"
        );
        assert_eq!(
            connection
                .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            5_000
        );
    }

    #[test]
    fn table_columns_match_go_schema_exactly() {
        let (_directory, _path, store) = open_temp_store();
        let connection = store.connection();

        assert_eq!(
            columns(connection, "accounts"),
            vec![
                column("id", "TEXT", false, None, 1),
                column("server", "TEXT", true, None, 0),
                column("remote_starred_total", "INTEGER", true, Some("0"), 0),
                column("last_sync_at", "TEXT", false, None, 0),
            ]
        );
        assert_eq!(
            columns(connection, "categories"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("id", "INTEGER", true, None, 2),
                column("title", "TEXT", true, None, 0),
            ]
        );
        assert_eq!(
            columns(connection, "feeds"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("id", "INTEGER", true, None, 2),
                column("category_id", "INTEGER", true, None, 0),
                column("title", "TEXT", true, None, 0),
                column("remote_unread_count", "INTEGER", true, Some("0"), 0),
            ]
        );
        assert_eq!(
            columns(connection, "selection_totals"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("kind", "TEXT", true, None, 2),
                column("selection_id", "INTEGER", true, Some("0"), 3),
                column("unread_only", "INTEGER", true, None, 4),
                column("total", "INTEGER", true, None, 0),
            ]
        );
        assert_eq!(
            columns(connection, "entries"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("id", "INTEGER", true, None, 2),
                column("title", "TEXT", true, None, 0),
                column("url", "TEXT", true, None, 0),
                column("comments_url", "TEXT", true, Some("''"), 0),
                column("feed_id", "INTEGER", true, None, 0),
                column("feed_name", "TEXT", true, None, 0),
                column("category_id", "INTEGER", true, Some("0"), 0),
                column("published_at", "TEXT", true, None, 0),
                column("preview", "TEXT", true, Some("''"), 0),
                column("image_url", "TEXT", true, Some("''"), 0),
                column("remote_status", "TEXT", true, None, 0),
                column("remote_starred", "INTEGER", true, None, 0),
                column("status", "TEXT", true, None, 0),
                column("starred", "INTEGER", true, None, 0),
            ]
        );
        assert_eq!(
            columns(connection, "pending_mutations"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("entry_id", "INTEGER", true, None, 2),
                column("field", "TEXT", true, None, 3),
                column("desired", "INTEGER", true, None, 0),
                column("revision", "INTEGER", true, None, 0),
                column("updated_at", "TEXT", true, None, 0),
            ]
        );
        assert_eq!(
            columns(connection, "undo_batches"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("id", "TEXT", true, None, 2),
                column("created_at", "TEXT", true, None, 0),
            ]
        );
        assert_eq!(
            columns(connection, "undo_items"),
            vec![
                column("account_id", "TEXT", true, None, 1),
                column("batch_id", "TEXT", true, None, 2),
                column("entry_id", "INTEGER", true, None, 3),
                column("prior_read", "INTEGER", true, None, 0),
            ]
        );
    }

    #[test]
    fn schema_defaults_constraints_and_absent_foreign_keys_match_go() {
        let (_directory, _path, store) = open_temp_store();
        let connection = store.connection();

        connection
            .execute(
                "INSERT INTO accounts(id,server) VALUES('a','https://a')",
                [],
            )
            .unwrap();
        let defaults: (i64, Option<String>) = connection
            .query_row(
                "SELECT remote_starred_total,last_sync_at FROM accounts WHERE id='a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(defaults, (0, None));

        let foreign_key_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_foreign_key_list('entries')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(foreign_key_count, 0);

        let invalid_field = connection.execute(
            "INSERT INTO pending_mutations(account_id,entry_id,field,desired,revision,updated_at)
             VALUES('a',1,'other',1,1,'2026-01-01T00:00:00Z')",
            [],
        );
        assert!(invalid_field.is_err());
    }

    #[test]
    fn ensure_account_updates_only_server() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("account", "https://old").unwrap();
        store
            .connection()
            .execute(
                "UPDATE accounts SET remote_starred_total=9,last_sync_at='2026-01-01T00:00:00Z'
                 WHERE id='account'",
                [],
            )
            .unwrap();

        store.ensure_account("account", "https://new").unwrap();
        let row: (String, i64, String) = store
            .connection()
            .query_row(
                "SELECT server,remote_starred_total,last_sync_at FROM accounts WHERE id='account'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "https://new".to_string(),
                9,
                "2026-01-01T00:00:00Z".to_string()
            )
        );
    }

    #[test]
    fn account_scoping_prevents_entry_leakage() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        store.ensure_account("b", "https://b").unwrap();
        let mut first = sample_entry(1, EntryStatus::Unread);
        let mut second = sample_entry(1, EntryStatus::Read);
        first.entry.title = "Account A".to_string();
        second.entry.title = "Account B".to_string();
        store.upsert_entry("a", &first).unwrap();
        store.upsert_entry("b", &second).unwrap();

        assert_eq!(
            store.entry("a", 1).unwrap().unwrap().entry.title,
            "Account A"
        );
        assert_eq!(
            store.entry("b", 1).unwrap().unwrap().entry.title,
            "Account B"
        );
        assert!(store.entry("c", 1).unwrap().is_none());
    }

    #[test]
    fn unknown_status_and_row_encodings_round_trip_without_loss() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        let record = PersistedEntry {
            remote_status: EntryStatus::Other("future-remote".to_string()),
            ..sample_entry(7, EntryStatus::Other("future-local".to_string()))
        };
        store.upsert_entry("a", &record).unwrap();

        let loaded = store.entry("a", 7).unwrap().unwrap();
        assert_eq!(loaded, record);
        let raw: (String, String, i64, i64, String) = store
            .connection()
            .query_row(
                "SELECT remote_status,status,remote_starred,starred,published_at
                 FROM entries WHERE account_id='a' AND id=7",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            raw,
            (
                "future-remote".to_string(),
                "future-local".to_string(),
                0,
                1,
                "2026-08-20T10:00:00.123456789Z".to_string(),
            )
        );

        // An unrelated mutation must not normalize a forward-compatible
        // status accepted by the Go schema.
        store.set_starred("a", 7, false).unwrap();
        assert_eq!(
            store.entry("a", 7).unwrap().unwrap().entry.status.as_str(),
            "future-local"
        );
    }

    #[test]
    fn primary_keys_and_transactions_are_atomic() {
        let (_directory, _path, store) = open_temp_store();
        let connection = store.connection();
        connection.execute_batch("BEGIN").unwrap();
        connection
            .execute(
                "INSERT INTO accounts(id,server) VALUES('a','https://a')",
                [],
            )
            .unwrap();
        let duplicate = connection.execute(
            "INSERT INTO accounts(id,server) VALUES('a','https://duplicate')",
            [],
        );
        assert!(duplicate.is_err());
        connection.execute_batch("ROLLBACK").unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn opening_partial_schema_adds_missing_objects_without_version_metadata() {
        let directory = TestDirectory::new();
        let path = directory.path().join("inbox.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE accounts (
                   id TEXT PRIMARY KEY,
                   server TEXT NOT NULL,
                   remote_starred_total INTEGER NOT NULL DEFAULT 0,
                   last_sync_at TEXT
                 );
                 INSERT INTO accounts(id,server) VALUES('existing','https://existing');",
            )
            .unwrap();
        drop(connection);

        let store = Store::open(&path).unwrap();
        let server: String = store
            .connection()
            .query_row(
                "SELECT server FROM accounts WHERE id='existing'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(server, "https://existing");
        let feeds_exists: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='feeds'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(feeds_exists, 1);
        assert_eq!(
            store
                .connection()
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn local_snapshot_selections_limits_and_retention() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        let connection = store.connection();
        connection
            .execute_batch(
                "INSERT INTO categories(account_id,id,title) VALUES('a',10,'Tech'),('a',20,'news');
                 INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count) VALUES
                   ('a',100,10,'Alpha Feed',2),('a',101,10,'alpha feed',1),('a',200,20,'Daily',3),
                   ('a',300,99,'Orphan',7);
                 INSERT INTO entries(account_id,id,title,url,comments_url,feed_id,feed_name,
                   category_id,published_at,preview,image_url,remote_status,remote_starred,status,starred) VALUES
                   ('a',1,'Unread One','u1','',100,'Alpha Feed',10,'2026-08-22T10:00:00Z','p1','','unread',0,'unread',0),
                   ('a',2,'Read Starred','u2','',100,'Alpha Feed',10,'2026-08-22T10:00:01Z','p2','','read',0,'read',1),
                   ('a',3,'Unknown','u3','',200,'Daily',20,'2026-08-22T10:00:02Z','p3','','legacy',0,'legacy',1);",
            )
            .unwrap();

        let all = store
            .local_snapshot(
                "a",
                &Selection::All {
                    id: 0,
                    unread_only: false,
                },
                false,
                &[],
            )
            .unwrap();
        assert_eq!(all.entries.len(), 3);
        assert_eq!(
            all.selection,
            Selection::All {
                id: 0,
                unread_only: false
            }
        );
        // Navigation: orphan feed excluded; NOCASE ordering groups the two
        // alpha feeds with input order preserved on ties.
        assert_eq!(all.categories.len(), 2);
        assert_eq!(all.categories[0].title, "news");
        assert_eq!(all.categories[1].feeds.len(), 2);
        assert_eq!(all.categories[1].feeds[0].id, 100);
        assert_eq!(all.categories[1].feeds[0].unread_count, 2);

        let unread = store
            .local_snapshot("a", &Selection::normalize("unread", 0, false), false, &[])
            .unwrap();
        assert_eq!(
            unread.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![1]
        );

        // Retention: retained IDs are ORed into the selection, so the locally
        // read entry appears alongside still-unread rows (Go behavior).
        let retained = store
            .local_snapshot("a", &Selection::normalize("unread", 0, true), false, &[2])
            .unwrap();
        assert_eq!(
            retained.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![1, 2]
        );

        // Unknown status survives a snapshot round trip.
        let unknown = &all.entries[2];
        assert_eq!(unknown.status.as_str(), "legacy");
    }

    #[test]
    fn local_snapshot_enforces_200_row_limit_with_ordering() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        let mut sql = String::from(
            "INSERT INTO entries(account_id,id,title,url,comments_url,feed_id,feed_name,category_id,published_at,preview,image_url,remote_status,remote_starred,status,starred) VALUES",
        );
        for id in 0..250i64 {
            if id > 0 {
                sql.push(',');
            }
            sql.push_str(&format!(
                "('a',{id},'t','u','','1','f',0,'2026-01-01T00:{:02}:{:02}Z','p','','unread',0,'unread',0)",
                id / 60 % 60,
                id % 60
            ));
        }
        store.connection().execute_batch(&sql).unwrap();

        let newest = store
            .local_snapshot(
                "a",
                &Selection::All {
                    id: 0,
                    unread_only: false,
                },
                true,
                &[],
            )
            .unwrap();
        assert_eq!(newest.entries.len(), SNAPSHOT_LIMIT as usize);
        assert_eq!(newest.entries[0].id, 249, "newest first");

        let oldest = store
            .local_snapshot(
                "a",
                &Selection::All {
                    id: 0,
                    unread_only: false,
                },
                false,
                &[],
            )
            .unwrap();
        assert_eq!(oldest.entries[0].id, 0, "oldest first");

        let total_override = store
            .local_snapshot(
                "a",
                &Selection::All {
                    id: 0,
                    unread_only: false,
                },
                false,
                &[],
            )
            .unwrap();
        assert_eq!(total_override.total, 250, "COUNT(*) without totals row");
    }

    #[test]
    fn local_snapshot_applies_remote_total_override_with_pending_delta() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        let connection = store.connection();
        connection
            .execute_batch(
                "INSERT INTO entries(account_id,id,title,url,comments_url,feed_id,feed_name,category_id,published_at,preview,image_url,remote_status,remote_starred,status,starred)
                 VALUES('a',4,'E','u','','1','f',0,'2026-01-01T00:00:04Z','p','','read',0,'unread',0);
                 INSERT INTO pending_mutations(account_id,entry_id,field,desired,revision,updated_at)
                 VALUES('a',4,'read',1,1,'2026-01-01T00:00:05Z');
                 INSERT INTO selection_totals(account_id,kind,selection_id,unread_only,total)
                 VALUES('a','all',0,1,50);
                 UPDATE accounts SET remote_starred_total=9 WHERE id='a';",
            )
            .unwrap();

        let data = store
            .local_snapshot(
                "a",
                &Selection::All {
                    id: 0,
                    unread_only: true,
                },
                false,
                &[],
            )
            .unwrap();
        // COUNT(*)=1; remote total 50 + pending read divergence (+1) wins.
        assert_eq!(data.total, 51);
        // Starred total: remote 9, no starred pendings.
        assert_eq!(data.starred_total, 9);

        // Missing account makes starredTotal fail like Go (ErrNoRows).
        let missing = store.local_snapshot(
            "missing",
            &Selection::All {
                id: 0,
                unread_only: false,
            },
            false,
            &[],
        );
        assert!(missing.is_err());
    }

    #[test]
    fn local_snapshot_isolates_accounts_end_to_end() {
        let (_directory, _path, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        store.ensure_account("b", "https://b").unwrap();
        for account in ["a", "b"] {
            let mut record = sample_entry(1, EntryStatus::Unread);
            record.entry.title = format!("Owned by {account}");
            store.upsert_entry(account, &record).unwrap();
        }
        for account in ["a", "b"] {
            let data = store
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
            assert_eq!(data.entries.len(), 1);
            assert_eq!(data.entries[0].title, format!("Owned by {account}"));
        }
    }

    #[test]
    fn probe_override() {
        let (_d, _p, store) = open_temp_store();
        store.ensure_account("a", "https://a").unwrap();
        store.connection().execute_batch(
        "INSERT INTO selection_totals(account_id,kind,selection_id,unread_only,total) VALUES('a','all',0,1,50);",
    ).unwrap();
        let sel = Selection::All {
            id: 0,
            unread_only: true,
        };
        println!(
            "total_row={:?}",
            store.selection_total("a", "all", 0, true).unwrap()
        );
        let data = store.local_snapshot("a", &sel, false, &[]).unwrap();
        println!("data_total={}", data.total);
    }

    #[cfg(unix)]
    #[test]
    fn database_file_mode_is_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (_directory, path, _store) = open_temp_store();
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Column {
        name: String,
        declared_type: String,
        not_null: bool,
        default_value: Option<String>,
        primary_key_order: i64,
    }

    fn columns(connection: &Connection, table: &str) -> Vec<Column> {
        let quoted = table.replace('"', "\"\"");
        connection
            .prepare(&format!("PRAGMA table_info(\"{quoted}\")"))
            .unwrap()
            .query_map([], |row| {
                Ok(Column {
                    name: row.get(1)?,
                    declared_type: row.get(2)?,
                    not_null: row.get::<_, i64>(3)? != 0,
                    default_value: row.get(4)?,
                    primary_key_order: row.get(5)?,
                })
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn column(
        name: &str,
        declared_type: &str,
        not_null: bool,
        default_value: Option<&str>,
        primary_key_order: i64,
    ) -> Column {
        Column {
            name: name.to_string(),
            declared_type: declared_type.to_string(),
            not_null,
            default_value: default_value.map(str::to_string),
            primary_key_order,
        }
    }

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};

            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "fluxbar-rust-store-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
