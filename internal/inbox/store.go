package inbox

import (
	"context"
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/model"
	_ "github.com/mattn/go-sqlite3"
)

const snapshotLimit = 200

type Store struct {
	db *sql.DB
}

type MutationReceipt struct {
	ID    string `json:"id"`
	Count int    `json:"count"`
}

type PendingMutation struct {
	EntryID  int64
	Field    string
	Desired  bool
	Revision int64
}

func OpenStore(path string) (*Store, error) {
	db, err := sql.Open("sqlite3", path+"?_busy_timeout=5000&_foreign_keys=on&_journal_mode=WAL&_synchronous=NORMAL")
	if err != nil {
		return nil, fmt.Errorf("SQLite öffnen: %w", err)
	}
	db.SetMaxOpenConns(1)
	store := &Store{db: db}
	if err := store.migrate(); err != nil {
		db.Close()
		return nil, err
	}
	if err := os.Chmod(path, 0o600); err != nil {
		db.Close()
		return nil, fmt.Errorf("SQLite-Dateirechte setzen: %w", err)
	}
	return store, nil
}

func (store *Store) Close() error { return store.db.Close() }

func (store *Store) migrate() error {
	const schema = `
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
);`
	if _, err := store.db.Exec(schema); err != nil {
		return fmt.Errorf("SQLite-Schema anlegen: %w", err)
	}
	return nil
}

func (store *Store) EnsureAccount(ctx context.Context, accountID, server string) error {
	_, err := store.db.ExecContext(ctx, `INSERT INTO accounts(id, server) VALUES(?, ?)
ON CONFLICT(id) DO UPDATE SET server=excluded.server`, accountID, server)
	if err != nil {
		return fmt.Errorf("SQLite-Konto anlegen: %w", err)
	}
	return nil
}

func (store *Store) ApplySnapshot(ctx context.Context, accountID string, snapshot model.BrowseSnapshot) error {
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if _, err = tx.ExecContext(ctx, `UPDATE accounts SET remote_starred_total=?, last_sync_at=? WHERE id=?`, snapshot.StarredTotal, time.Now().UTC().Format(time.RFC3339Nano), accountID); err != nil {
		return fmt.Errorf("Sync-Status speichern: %w", err)
	}
	selection := snapshot.Selection.Normalized()
	if _, err = tx.ExecContext(ctx, `INSERT INTO selection_totals(account_id,kind,selection_id,unread_only,total) VALUES(?,?,?,?,?)
ON CONFLICT(account_id,kind,selection_id,unread_only) DO UPDATE SET total=excluded.total`, accountID, selection.Kind, selection.ID, selection.UnreadOnly, snapshot.Total); err != nil {
		return fmt.Errorf("Auswahlzähler speichern: %w", err)
	}
	if snapshot.Total <= len(snapshot.Entries) && (selection.UnreadOnly || selection.Kind == model.SelectionUnread || selection.Kind == model.SelectionStarred) {
		if err := reconcileCompleteSelection(ctx, tx, accountID, selection, snapshot.Entries); err != nil {
			return err
		}
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM feeds WHERE account_id=?`, accountID); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM categories WHERE account_id=?`, accountID); err != nil {
		return err
	}
	for _, category := range snapshot.Categories {
		if _, err = tx.ExecContext(ctx, `INSERT INTO categories(account_id,id,title) VALUES(?,?,?)`, accountID, category.ID, category.Title); err != nil {
			return err
		}
		for _, feed := range category.Feeds {
			if _, err = tx.ExecContext(ctx, `INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count) VALUES(?,?,?,?,?)`, accountID, feed.ID, category.ID, feed.Title, feed.UnreadCount); err != nil {
				return err
			}
		}
	}
	for _, entry := range snapshot.Entries {
		var pendingRead, pendingStarred int
		_ = tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM pending_mutations WHERE account_id=? AND entry_id=? AND field='read'`, accountID, entry.ID).Scan(&pendingRead)
		_ = tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM pending_mutations WHERE account_id=? AND entry_id=? AND field='starred'`, accountID, entry.ID).Scan(&pendingStarred)
		_, err = tx.ExecContext(ctx, `INSERT INTO entries(
account_id,id,title,url,comments_url,feed_id,feed_name,category_id,published_at,preview,image_url,
remote_status,remote_starred,status,starred) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
ON CONFLICT(account_id,id) DO UPDATE SET
title=excluded.title,url=excluded.url,comments_url=excluded.comments_url,feed_id=excluded.feed_id,
feed_name=excluded.feed_name,category_id=excluded.category_id,published_at=excluded.published_at,
preview=excluded.preview,image_url=excluded.image_url,remote_status=excluded.remote_status,
remote_starred=excluded.remote_starred,
status=CASE WHEN ? THEN entries.status ELSE excluded.status END,
starred=CASE WHEN ? THEN entries.starred ELSE excluded.starred END`,
			accountID, entry.ID, entry.Title, entry.URL, entry.CommentsURL, entry.FeedID, entry.FeedName,
			entry.CategoryID, entry.PublishedAt.UTC().Format(time.RFC3339Nano), entry.Preview, entry.ImageURL,
			entry.Status, entry.Starred, entry.Status, entry.Starred, pendingRead > 0, pendingStarred > 0)
		if err != nil {
			return fmt.Errorf("Artikel %d speichern: %w", entry.ID, err)
		}
	}
	if err = tx.Commit(); err != nil {
		return fmt.Errorf("SQLite-Snapshot abschließen: %w", err)
	}
	return nil
}

func (store *Store) Snapshot(ctx context.Context, accountID string, selection model.Selection, newestFirst bool, retainIDs []int64) (model.BrowseSnapshot, error) {
	selection = selection.Normalized()
	categories, unreadTotal, err := store.navigation(ctx, accountID)
	if err != nil {
		return model.BrowseSnapshot{}, err
	}
	starredTotal, err := store.starredTotal(ctx, accountID)
	if err != nil {
		return model.BrowseSnapshot{}, err
	}
	where, arguments := selectionClause(selection, accountID)
	countQuery := "SELECT COUNT(*) FROM entries WHERE " + where
	var total int
	if err := store.db.QueryRowContext(ctx, countQuery, arguments...).Scan(&total); err != nil {
		return model.BrowseSnapshot{}, fmt.Errorf("lokale Artikel zählen: %w", err)
	}
	var remoteTotal int
	if err := store.db.QueryRowContext(ctx, `SELECT total FROM selection_totals WHERE account_id=? AND kind=? AND selection_id=? AND unread_only=?`, accountID, selection.Kind, selection.ID, selection.UnreadOnly).Scan(&remoteTotal); err == nil {
		delta, deltaErr := store.selectionPendingDelta(ctx, accountID, selection)
		if deltaErr != nil {
			return model.BrowseSnapshot{}, deltaErr
		}
		total = max(total, max(0, remoteTotal+delta))
	} else if err != sql.ErrNoRows {
		return model.BrowseSnapshot{}, err
	}

	entryWhere := where
	entryArguments := append([]any(nil), arguments...)
	if len(retainIDs) > 0 {
		placeholders := make([]string, 0, len(retainIDs))
		for range retainIDs {
			placeholders = append(placeholders, "?")
		}
		entryWhere = "(" + where + ") OR (account_id=? AND id IN (" + strings.Join(placeholders, ",") + "))"
		entryArguments = append(arguments, accountID)
		for _, id := range retainIDs {
			entryArguments = append(entryArguments, id)
		}
	}
	direction := "ASC"
	if newestFirst {
		direction = "DESC"
	}
	query := `SELECT id,title,url,comments_url,feed_id,feed_name,category_id,published_at,preview,image_url,status,starred
FROM entries WHERE ` + entryWhere + ` ORDER BY published_at ` + direction + ` LIMIT ` + fmt.Sprint(snapshotLimit)
	rows, err := store.db.QueryContext(ctx, query, entryArguments...)
	if err != nil {
		return model.BrowseSnapshot{}, fmt.Errorf("lokale Artikel laden: %w", err)
	}
	defer rows.Close()
	entries := make([]model.Entry, 0)
	seen := make(map[int64]bool)
	for rows.Next() {
		var entry model.Entry
		var published string
		if err := rows.Scan(&entry.ID, &entry.Title, &entry.URL, &entry.CommentsURL, &entry.FeedID, &entry.FeedName, &entry.CategoryID, &published, &entry.Preview, &entry.ImageURL, &entry.Status, &entry.Starred); err != nil {
			return model.BrowseSnapshot{}, err
		}
		entry.PublishedAt, _ = time.Parse(time.RFC3339Nano, published)
		if !seen[entry.ID] {
			entries = append(entries, entry)
			seen[entry.ID] = true
		}
	}
	return model.BrowseSnapshot{Version: 1, Selection: selection, Entries: entries, Categories: categories, Total: total, UnreadTotal: unreadTotal, StarredTotal: starredTotal}, rows.Err()
}

func reconcileCompleteSelection(ctx context.Context, tx *sql.Tx, accountID string, selection model.Selection, entries []model.Entry) error {
	parts := []string{"account_id=?"}
	arguments := []any{accountID}
	if selection.Kind == model.SelectionCategory {
		parts = append(parts, "category_id=?")
		arguments = append(arguments, selection.ID)
	}
	if selection.Kind == model.SelectionFeed {
		parts = append(parts, "feed_id=?")
		arguments = append(arguments, selection.ID)
	}
	if len(entries) > 0 {
		placeholders := make([]string, len(entries))
		for index, entry := range entries {
			placeholders[index] = "?"
			arguments = append(arguments, entry.ID)
		}
		parts = append(parts, "id NOT IN ("+strings.Join(placeholders, ",")+")")
	}
	where := strings.Join(parts, " AND ")
	if selection.UnreadOnly || selection.Kind == model.SelectionUnread {
		_, err := tx.ExecContext(ctx, `UPDATE entries SET remote_status='read',status=CASE WHEN EXISTS(
SELECT 1 FROM pending_mutations p WHERE p.account_id=entries.account_id AND p.entry_id=entries.id AND p.field='read'
) THEN status ELSE 'read' END WHERE `+where+` AND remote_status='unread'`, arguments...)
		return err
	}
	if selection.Kind == model.SelectionStarred {
		_, err := tx.ExecContext(ctx, `UPDATE entries SET remote_starred=0,starred=CASE WHEN EXISTS(
SELECT 1 FROM pending_mutations p WHERE p.account_id=entries.account_id AND p.entry_id=entries.id AND p.field='starred'
) THEN starred ELSE 0 END WHERE `+where+` AND remote_starred=1`, arguments...)
		return err
	}
	return nil
}

func (store *Store) selectionPendingDelta(ctx context.Context, accountID string, selection model.Selection) (int, error) {
	field := ""
	expression := ""
	if selection.UnreadOnly || selection.Kind == model.SelectionUnread {
		field = "read"
		expression = "CASE WHEN e.status='unread' AND e.remote_status='read' THEN 1 WHEN e.status='read' AND e.remote_status='unread' THEN -1 ELSE 0 END"
	} else if selection.Kind == model.SelectionStarred {
		field = "starred"
		expression = "CASE WHEN e.starred=1 AND e.remote_starred=0 THEN 1 WHEN e.starred=0 AND e.remote_starred=1 THEN -1 ELSE 0 END"
	} else {
		return 0, nil
	}
	query := `SELECT COALESCE(SUM(` + expression + `),0) FROM entries e JOIN pending_mutations p
ON p.account_id=e.account_id AND p.entry_id=e.id AND p.field=? WHERE e.account_id=?`
	arguments := []any{field, accountID}
	if selection.Kind == model.SelectionCategory {
		query += " AND e.category_id=?"
		arguments = append(arguments, selection.ID)
	}
	if selection.Kind == model.SelectionFeed {
		query += " AND e.feed_id=?"
		arguments = append(arguments, selection.ID)
	}
	var delta int
	err := store.db.QueryRowContext(ctx, query, arguments...).Scan(&delta)
	return delta, err
}

func selectionClause(selection model.Selection, accountID string) (string, []any) {
	parts := []string{"account_id=?"}
	arguments := []any{accountID}
	if selection.UnreadOnly || selection.Kind == model.SelectionUnread {
		parts = append(parts, "status='unread'")
	}
	switch selection.Kind {
	case model.SelectionStarred:
		parts = append(parts, "starred=1")
	case model.SelectionCategory:
		parts = append(parts, "category_id=?")
		arguments = append(arguments, selection.ID)
	case model.SelectionFeed:
		parts = append(parts, "feed_id=?")
		arguments = append(arguments, selection.ID)
	}
	return strings.Join(parts, " AND "), arguments
}

func (store *Store) navigation(ctx context.Context, accountID string) ([]model.Category, int, error) {
	rows, err := store.db.QueryContext(ctx, `SELECT id,title FROM categories WHERE account_id=? ORDER BY title COLLATE NOCASE`, accountID)
	if err != nil {
		return nil, 0, err
	}
	var categories []model.Category
	for rows.Next() {
		var category model.Category
		if err := rows.Scan(&category.ID, &category.Title); err != nil {
			rows.Close()
			return nil, 0, err
		}
		category.Feeds = []model.Feed{}
		categories = append(categories, category)
	}
	rows.Close()
	unreadTotal := 0
	for index := range categories {
		feedRows, err := store.db.QueryContext(ctx, `SELECT id,title,remote_unread_count FROM feeds WHERE account_id=? AND category_id=? ORDER BY title COLLATE NOCASE`, accountID, categories[index].ID)
		if err != nil {
			return nil, 0, err
		}
		type storedFeed struct {
			feed        model.Feed
			remoteCount int
		}
		var storedFeeds []storedFeed
		for feedRows.Next() {
			var value storedFeed
			if err := feedRows.Scan(&value.feed.ID, &value.feed.Title, &value.remoteCount); err != nil {
				feedRows.Close()
				return nil, 0, err
			}
			value.feed.CategoryID = categories[index].ID
			storedFeeds = append(storedFeeds, value)
		}
		feedRows.Close()
		for _, value := range storedFeeds {
			feed := value.feed
			var delta int
			_ = store.db.QueryRowContext(ctx, `SELECT COALESCE(SUM(CASE WHEN e.status='unread' AND e.remote_status='read' THEN 1 WHEN e.status='read' AND e.remote_status='unread' THEN -1 ELSE 0 END),0)
FROM entries e JOIN pending_mutations p ON p.account_id=e.account_id AND p.entry_id=e.id AND p.field='read'
WHERE e.account_id=? AND e.feed_id=?`, accountID, feed.ID).Scan(&delta)
			feed.UnreadCount = max(0, value.remoteCount+delta)
			categories[index].UnreadCount += feed.UnreadCount
			unreadTotal += feed.UnreadCount
			categories[index].Feeds = append(categories[index].Feeds, feed)
		}
	}
	return categories, unreadTotal, nil
}

func (store *Store) starredTotal(ctx context.Context, accountID string) (int, error) {
	var remoteTotal, delta int
	if err := store.db.QueryRowContext(ctx, `SELECT remote_starred_total FROM accounts WHERE id=?`, accountID).Scan(&remoteTotal); err != nil {
		return 0, err
	}
	if err := store.db.QueryRowContext(ctx, `SELECT COALESCE(SUM(CASE WHEN e.starred=1 AND e.remote_starred=0 THEN 1 WHEN e.starred=0 AND e.remote_starred=1 THEN -1 ELSE 0 END),0)
FROM entries e JOIN pending_mutations p ON p.account_id=e.account_id AND p.entry_id=e.id AND p.field='starred' WHERE e.account_id=?`, accountID).Scan(&delta); err != nil {
		return 0, err
	}
	return max(0, remoteTotal+delta), nil
}

func (store *Store) SetRead(ctx context.Context, accountID string, entryIDs []int64, read, undoable bool) (*MutationReceipt, error) {
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	receipt := &MutationReceipt{ID: randomID()}
	if undoable {
		if _, err := tx.ExecContext(ctx, `INSERT INTO undo_batches(account_id,id,created_at) VALUES(?,?,?)`, accountID, receipt.ID, time.Now().UTC().Format(time.RFC3339Nano)); err != nil {
			return nil, err
		}
	}
	status := "unread"
	if read {
		status = "read"
	}
	for _, entryID := range entryIDs {
		if !undoable {
			if _, err = tx.ExecContext(ctx, `DELETE FROM undo_items WHERE account_id=? AND entry_id=?`, accountID, entryID); err != nil {
				return nil, err
			}
		}
		var current string
		if err := tx.QueryRowContext(ctx, `SELECT status FROM entries WHERE account_id=? AND id=?`, accountID, entryID).Scan(&current); err != nil {
			if err == sql.ErrNoRows {
				continue
			}
			return nil, err
		}
		if current == status {
			continue
		}
		if undoable {
			_, err = tx.ExecContext(ctx, `INSERT INTO undo_items(account_id,batch_id,entry_id,prior_read) VALUES(?,?,?,?)`, accountID, receipt.ID, entryID, current == "read")
			if err != nil {
				return nil, err
			}
		}
		if _, err = tx.ExecContext(ctx, `UPDATE entries SET status=? WHERE account_id=? AND id=?`, status, accountID, entryID); err != nil {
			return nil, err
		}
		if err = upsertPending(ctx, tx, accountID, entryID, "read", read); err != nil {
			return nil, err
		}
		receipt.Count++
	}
	if undoable && receipt.Count == 0 {
		if _, err := tx.ExecContext(ctx, `DELETE FROM undo_batches WHERE account_id=? AND id=?`, accountID, receipt.ID); err != nil {
			return nil, err
		}
	}
	if !undoable {
		if _, err = tx.ExecContext(ctx, `DELETE FROM undo_batches WHERE account_id=? AND NOT EXISTS(
SELECT 1 FROM undo_items i WHERE i.account_id=undo_batches.account_id AND i.batch_id=undo_batches.id)`, accountID); err != nil {
			return nil, err
		}
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	if !undoable || receipt.Count == 0 {
		return nil, nil
	}
	return receipt, nil
}

func (store *Store) DiscardUndo(ctx context.Context, accountID, batchID string) error {
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `DELETE FROM undo_items WHERE account_id=? AND batch_id=?`, accountID, batchID); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM undo_batches WHERE account_id=? AND id=?`, accountID, batchID); err != nil {
		return err
	}
	return tx.Commit()
}

func (store *Store) SetStarred(ctx context.Context, accountID string, entryID int64, starred bool) error {
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `UPDATE entries SET starred=? WHERE account_id=? AND id=?`, starred, accountID, entryID); err != nil {
		return err
	}
	if err = upsertPending(ctx, tx, accountID, entryID, "starred", starred); err != nil {
		return err
	}
	return tx.Commit()
}

func upsertPending(ctx context.Context, tx *sql.Tx, accountID string, entryID int64, field string, desired bool) error {
	_, err := tx.ExecContext(ctx, `INSERT INTO pending_mutations(account_id,entry_id,field,desired,revision,updated_at) VALUES(?,?,?,?,1,?)
ON CONFLICT(account_id,entry_id,field) DO UPDATE SET desired=excluded.desired,revision=pending_mutations.revision+1,updated_at=excluded.updated_at`,
		accountID, entryID, field, desired, time.Now().UTC().Format(time.RFC3339Nano))
	return err
}

func (store *Store) Undo(ctx context.Context, accountID, batchID string) ([]int64, error) {
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	rows, err := tx.QueryContext(ctx, `SELECT entry_id,prior_read FROM undo_items WHERE account_id=? AND batch_id=?`, accountID, batchID)
	if err != nil {
		return nil, err
	}
	type item struct {
		id    int64
		prior bool
	}
	var items []item
	for rows.Next() {
		var value item
		if err := rows.Scan(&value.id, &value.prior); err != nil {
			rows.Close()
			return nil, err
		}
		items = append(items, value)
	}
	rows.Close()
	for _, item := range items {
		status := "unread"
		if item.prior {
			status = "read"
		}
		if _, err = tx.ExecContext(ctx, `UPDATE entries SET status=? WHERE account_id=? AND id=?`, status, accountID, item.id); err != nil {
			return nil, err
		}
		if err = upsertPending(ctx, tx, accountID, item.id, "read", item.prior); err != nil {
			return nil, err
		}
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM undo_items WHERE account_id=? AND batch_id=?`, accountID, batchID); err != nil {
		return nil, err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM undo_batches WHERE account_id=? AND id=?`, accountID, batchID); err != nil {
		return nil, err
	}
	if err = tx.Commit(); err != nil {
		return nil, err
	}
	ids := make([]int64, len(items))
	for index, item := range items {
		ids[index] = item.id
	}
	return ids, nil
}

func (store *Store) Pending(ctx context.Context, accountID string) ([]PendingMutation, error) {
	rows, err := store.db.QueryContext(ctx, `SELECT entry_id,field,desired,revision FROM pending_mutations WHERE account_id=? ORDER BY updated_at`, accountID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var pending []PendingMutation
	for rows.Next() {
		var mutation PendingMutation
		if err := rows.Scan(&mutation.EntryID, &mutation.Field, &mutation.Desired, &mutation.Revision); err != nil {
			return nil, err
		}
		pending = append(pending, mutation)
	}
	return pending, rows.Err()
}

func (store *Store) Acknowledge(ctx context.Context, accountID string, mutation PendingMutation) error {
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	column := "remote_status"
	value := any("unread")
	if mutation.Field == "read" {
		if mutation.Desired {
			value = "read"
		}
		var previous string
		var feedID, categoryID int64
		if err = tx.QueryRowContext(ctx, `SELECT remote_status,feed_id,category_id FROM entries WHERE account_id=? AND id=?`, accountID, mutation.EntryID).Scan(&previous, &feedID, &categoryID); err != nil {
			return err
		}
		delta := 0
		if previous == "unread" && mutation.Desired {
			delta = -1
		}
		if previous == "read" && !mutation.Desired {
			delta = 1
		}
		if delta != 0 {
			if _, err = tx.ExecContext(ctx, `UPDATE feeds SET remote_unread_count=MAX(0,remote_unread_count+?) WHERE account_id=? AND id=?`, delta, accountID, feedID); err != nil {
				return err
			}
			if _, err = tx.ExecContext(ctx, `UPDATE selection_totals SET total=MAX(0,total+?) WHERE account_id=? AND (
kind='unread' OR (unread_only=1 AND (kind='all' OR (kind='feed' AND selection_id=?) OR (kind='category' AND selection_id=?))))`, delta, accountID, feedID, categoryID); err != nil {
				return err
			}
		}
	} else {
		column = "remote_starred"
		value = mutation.Desired
		var previous bool
		if err = tx.QueryRowContext(ctx, `SELECT remote_starred FROM entries WHERE account_id=? AND id=?`, accountID, mutation.EntryID).Scan(&previous); err != nil {
			return err
		}
		delta := 0
		if !previous && mutation.Desired {
			delta = 1
		}
		if previous && !mutation.Desired {
			delta = -1
		}
		if delta != 0 {
			if _, err = tx.ExecContext(ctx, `UPDATE accounts SET remote_starred_total=MAX(0,remote_starred_total+?) WHERE id=?`, delta, accountID); err != nil {
				return err
			}
			if _, err = tx.ExecContext(ctx, `UPDATE selection_totals SET total=MAX(0,total+?) WHERE account_id=? AND kind='starred'`, delta, accountID); err != nil {
				return err
			}
		}
	}
	if _, err = tx.ExecContext(ctx, `UPDATE entries SET `+column+`=? WHERE account_id=? AND id=?`, value, accountID, mutation.EntryID); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM pending_mutations WHERE account_id=? AND entry_id=? AND field=? AND revision=?`, accountID, mutation.EntryID, mutation.Field, mutation.Revision); err != nil {
		return err
	}
	return tx.Commit()
}

func randomID() string {
	data := make([]byte, 16)
	if _, err := rand.Read(data); err != nil {
		return fmt.Sprintf("%d", time.Now().UnixNano())
	}
	return hex.EncodeToString(data)
}
