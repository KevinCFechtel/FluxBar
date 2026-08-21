package inbox

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

func TestStorePersistsLocalSnapshotAndUndo(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "inbox.sqlite3")
	store, err := OpenStore(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.EnsureAccount(ctx, "account", "https://example.com"); err != nil {
		t.Fatal(err)
	}
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	if err := store.Close(); err != nil {
		t.Fatal(err)
	}

	store, err = OpenStore(path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { store.Close() })
	snapshot, err := store.Snapshot(ctx, "account", model.Selection{Kind: model.SelectionAll, UnreadOnly: true}, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Entries) != 2 || snapshot.UnreadTotal != 2 {
		t.Fatalf("persisted snapshot = %#v", snapshot)
	}

	receipt, err := store.SetRead(ctx, "account", []int64{1}, true, true)
	if err != nil {
		t.Fatal(err)
	}
	if receipt == nil || receipt.Count != 1 {
		t.Fatalf("receipt = %#v", receipt)
	}
	if err := store.Close(); err != nil {
		t.Fatal(err)
	}
	store, err = OpenStore(path)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err = store.Snapshot(ctx, "account", model.Selection{Kind: model.SelectionAll, UnreadOnly: true}, false, []int64{1})
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.UnreadTotal != 1 || len(snapshot.Entries) != 2 || snapshot.Entries[0].Status != "read" {
		t.Fatalf("mutated snapshot = %#v", snapshot)
	}
	if _, err := store.Undo(ctx, "account", receipt.ID); err != nil {
		t.Fatal(err)
	}
	snapshot, err = store.Snapshot(ctx, "account", model.Selection{Kind: model.SelectionAll, UnreadOnly: true}, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.UnreadTotal != 2 || len(snapshot.Entries) != 2 {
		t.Fatalf("undone snapshot = %#v", snapshot)
	}
}

func TestApplySnapshotPreservesPendingDesiredState(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	if _, err := store.SetRead(ctx, "account", []int64{1}, true, false); err != nil {
		t.Fatal(err)
	}
	if err := store.SetStarred(ctx, "account", 1, true); err != nil {
		t.Fatal(err)
	}

	remote := testSnapshot()
	remote.Entries[0].Status = "unread"
	remote.Entries[0].Starred = false
	if err := store.ApplySnapshot(ctx, "account", remote); err != nil {
		t.Fatal(err)
	}
	snapshot, err := store.Snapshot(ctx, "account", model.Selection{Kind: model.SelectionAll}, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Entries) != 2 || snapshot.Entries[0].Status != "read" || !snapshot.Entries[0].Starred {
		t.Fatalf("pending state overwritten: %#v", snapshot.Entries)
	}
}

func TestAcknowledgementKeepsNavigationCountsStable(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	if _, err := store.SetRead(ctx, "account", []int64{1}, true, false); err != nil {
		t.Fatal(err)
	}
	pending, err := store.Pending(ctx, "account")
	if err != nil || len(pending) != 1 {
		t.Fatalf("pending=%#v error=%v", pending, err)
	}
	if err := store.Acknowledge(ctx, "account", pending[0]); err != nil {
		t.Fatal(err)
	}
	snapshot, err := store.Snapshot(ctx, "account", model.Selection{Kind: model.SelectionAll, UnreadOnly: true}, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.UnreadTotal != 1 || snapshot.Total != 1 {
		t.Fatalf("counts after acknowledgement: unread=%d total=%d", snapshot.UnreadTotal, snapshot.Total)
	}
}

func TestSnapshotPreservesRemoteTotalBeyondCachedRows(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	remote := testSnapshot()
	remote.Total = 500
	if err := store.ApplySnapshot(ctx, "account", remote); err != nil {
		t.Fatal(err)
	}
	snapshot, err := store.Snapshot(ctx, "account", remote.Selection, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.Total != 500 || len(snapshot.Entries) != 2 {
		t.Fatalf("snapshot total=%d entries=%d", snapshot.Total, len(snapshot.Entries))
	}
}

func TestCompleteUnreadSnapshotReconcilesExternallyReadEntry(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	remote := testSnapshot()
	remote.Entries = remote.Entries[1:]
	remote.Total = 1
	remote.UnreadTotal = 1
	remote.Categories[0].UnreadCount = 1
	remote.Categories[0].Feeds[0].UnreadCount = 1
	if err := store.ApplySnapshot(ctx, "account", remote); err != nil {
		t.Fatal(err)
	}
	snapshot, err := store.Snapshot(ctx, "account", remote.Selection, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Entries) != 1 || snapshot.Entries[0].ID != 2 {
		t.Fatalf("reconciled entries = %#v", snapshot.Entries)
	}
}

func openTestStore(t *testing.T) *Store {
	t.Helper()
	store, err := OpenStore(filepath.Join(t.TempDir(), "inbox.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { store.Close() })
	if err := store.EnsureAccount(context.Background(), "account", "https://example.com"); err != nil {
		t.Fatal(err)
	}
	return store
}

func testSnapshot() model.BrowseSnapshot {
	category := model.Category{ID: 10, Title: "Category", UnreadCount: 2, Feeds: []model.Feed{{ID: 20, Title: "Feed", CategoryID: 10, UnreadCount: 2}}}
	return model.BrowseSnapshot{
		Version:   1,
		Selection: model.Selection{Kind: model.SelectionAll, UnreadOnly: true},
		Entries: []model.Entry{
			{ID: 1, Title: "One", URL: "https://example.com/1", FeedID: 20, FeedName: "Feed", CategoryID: 10, PublishedAt: time.Date(2026, 8, 20, 10, 0, 0, 0, time.UTC), Status: "unread"},
			{ID: 2, Title: "Two", URL: "https://example.com/2", FeedID: 20, FeedName: "Feed", CategoryID: 10, PublishedAt: time.Date(2026, 8, 20, 11, 0, 0, 0, time.UTC), Status: "unread"},
		},
		Categories: []model.Category{category}, Total: 2, UnreadTotal: 2,
	}
}
