//! Authoritative schema mirrored from `go-core/internal/inbox/store.go`.
//!
//! The Go implementation has no migration table or `user_version`; migration
//! consists of idempotent `CREATE ... IF NOT EXISTS` statements. Rust must not
//! introduce separate version metadata during compatibility migration.

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
  id TEXT PRIMARY KEY,
  server TEXT NOT NULL,
  remote_starred_total INTEGER NOT NULL DEFAULT 0,
  last_sync_at TEXT
);
CREATE TABLE IF NOT EXISTS categories (
  account_id TEXT NOT NULL,
  id INTEGER NOT NULL,
  title TEXT NOT NULL,
  PRIMARY KEY (account_id, id)
);
CREATE TABLE IF NOT EXISTS feeds (
  account_id TEXT NOT NULL,
  id INTEGER NOT NULL,
  category_id INTEGER NOT NULL,
  title TEXT NOT NULL,
  remote_unread_count INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account_id, id)
);
CREATE TABLE IF NOT EXISTS selection_totals (
  account_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  selection_id INTEGER NOT NULL DEFAULT 0,
  unread_only INTEGER NOT NULL,
  total INTEGER NOT NULL,
  PRIMARY KEY (account_id, kind, selection_id, unread_only)
);
CREATE TABLE IF NOT EXISTS entries (
  account_id TEXT NOT NULL,
  id INTEGER NOT NULL,
  title TEXT NOT NULL,
  url TEXT NOT NULL,
  comments_url TEXT NOT NULL DEFAULT '',
  feed_id INTEGER NOT NULL,
  feed_name TEXT NOT NULL,
  category_id INTEGER NOT NULL DEFAULT 0,
  published_at TEXT NOT NULL,
  preview TEXT NOT NULL DEFAULT '',
  image_url TEXT NOT NULL DEFAULT '',
  remote_status TEXT NOT NULL,
  remote_starred INTEGER NOT NULL,
  status TEXT NOT NULL,
  starred INTEGER NOT NULL,
  PRIMARY KEY (account_id, id)
);
CREATE INDEX IF NOT EXISTS entries_account_published ON entries(account_id, published_at);
CREATE INDEX IF NOT EXISTS entries_account_feed ON entries(account_id, feed_id, published_at);
CREATE INDEX IF NOT EXISTS entries_account_category ON entries(account_id, category_id, published_at);
CREATE TABLE IF NOT EXISTS pending_mutations (
  account_id TEXT NOT NULL,
  entry_id INTEGER NOT NULL,
  field TEXT NOT NULL CHECK(field IN ('read', 'starred')),
  desired INTEGER NOT NULL,
  revision INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (account_id, entry_id, field)
);
CREATE TABLE IF NOT EXISTS undo_batches (
  account_id TEXT NOT NULL,
  id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (account_id, id)
);
CREATE TABLE IF NOT EXISTS undo_items (
  account_id TEXT NOT NULL,
  batch_id TEXT NOT NULL,
  entry_id INTEGER NOT NULL,
  prior_read INTEGER NOT NULL,
  PRIMARY KEY (account_id, batch_id, entry_id)
);
"#;
