package inbox

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type fakeRemote struct {
	mu            sync.Mutex
	snapshot      model.BrowseSnapshot
	browseError   error
	browseEntered chan struct{}
	browseRelease chan struct{}
	readCalls     []struct {
		entryID int64
		read    bool
	}
	readErrorAt int
	starred     bool
	toggleCalls int
}

func (remote *fakeRemote) Browse(context.Context, model.Selection) (model.BrowseSnapshot, error) {
	remote.mu.Lock()
	snapshot, browseError := remote.snapshot, remote.browseError
	entered, release := remote.browseEntered, remote.browseRelease
	remote.mu.Unlock()
	if entered != nil {
		entered <- struct{}{}
		<-release
	}
	if browseError != nil {
		return model.BrowseSnapshot{}, browseError
	}
	return snapshot, nil
}
func (remote *fakeRemote) SetReadBatch(_ context.Context, ids []int64, read bool) error {
	remote.mu.Lock()
	defer remote.mu.Unlock()
	remote.readCalls = append(remote.readCalls, struct {
		entryID int64
		read    bool
	}{ids[0], read})
	if remote.readErrorAt > 0 && len(remote.readCalls) == remote.readErrorAt {
		return errors.New("read failed")
	}
	return nil
}
func (remote *fakeRemote) EntryState(context.Context, int64) (RemoteEntryState, error) {
	remote.mu.Lock()
	defer remote.mu.Unlock()
	return RemoteEntryState{Starred: remote.starred}, nil
}
func (remote *fakeRemote) ToggleStarred(context.Context, int64) error {
	remote.mu.Lock()
	defer remote.mu.Unlock()
	remote.starred = !remote.starred
	remote.toggleCalls++
	return nil
}
func (*fakeRemote) FeedIcon(context.Context, int64, string) ([]byte, []byte) { return nil, nil }

func TestSyncWritesAndRetainsLocalSnapshotOffline(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	remote := &fakeRemote{snapshot: testSnapshot()}
	service := NewService(store, remote, "account", false, nil)
	if _, err := service.Sync(ctx, model.Selection{Kind: model.SelectionAll, UnreadOnly: true}); err != nil {
		t.Fatal(err)
	}
	remote.browseError = errors.New("offline")
	snapshot, err := service.Sync(ctx, model.Selection{Kind: model.SelectionAll, UnreadOnly: true})
	if err == nil {
		t.Fatal("offline sync returned no error")
	}
	if len(snapshot.Entries) != 2 {
		t.Fatalf("offline snapshot = %#v", snapshot)
	}
}

func TestStarredReconciliationUsesRemoteDesiredState(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	remote := &fakeRemote{snapshot: testSnapshot(), starred: true}
	service := NewService(store, remote, "account", false, nil)
	if err := store.SetStarred(ctx, "account", 1, true); err != nil {
		t.Fatal(err)
	}
	if err := service.Flush(ctx); err != nil {
		t.Fatal(err)
	}
	if remote.toggleCalls != 0 {
		t.Fatalf("matching remote state toggled %d times", remote.toggleCalls)
	}
	if err := store.SetStarred(ctx, "account", 1, false); err != nil {
		t.Fatal(err)
	}
	if err := service.Flush(ctx); err != nil {
		t.Fatal(err)
	}
	if remote.toggleCalls != 1 || remote.starred {
		t.Fatalf("remote starred=%t toggles=%d", remote.starred, remote.toggleCalls)
	}
}

func TestSequentialAutomaticReadBatchesRetainEarlierRows(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	service := NewService(store, &fakeRemote{snapshot: testSnapshot()}, "account", false, nil)
	selection := model.Selection{Kind: model.SelectionAll, UnreadOnly: true}

	first, _, err := service.MarkRead(ctx, selection, []int64{1}, nil, true, true)
	if err != nil {
		t.Fatal(err)
	}
	if len(first.Entries) != 2 || first.Total != 1 {
		t.Fatalf("first batch: entries=%#v total=%d", first.Entries, first.Total)
	}
	second, _, err := service.MarkRead(ctx, selection, []int64{2}, []int64{1}, true, true)
	if err != nil {
		t.Fatal(err)
	}
	if len(second.Entries) != 2 || second.Total != 0 || second.Entries[0].Status != "read" || second.Entries[1].Status != "read" {
		t.Fatalf("second batch: entries=%#v total=%d", second.Entries, second.Total)
	}
}

func TestFlushAcknowledgesSuccessfulPrefixAndStopsAtFirstFailure(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	if _, err := store.SetRead(ctx, "account", []int64{1, 2}, true, false); err != nil {
		t.Fatal(err)
	}
	remote := &fakeRemote{readErrorAt: 2}
	service := NewService(store, remote, "account", false, nil)
	if err := service.Flush(ctx); err == nil || err.Error() != "read failed" {
		t.Fatalf("flush error = %v", err)
	}
	pending, err := store.Pending(ctx, "account")
	if err != nil {
		t.Fatal(err)
	}
	acknowledgedID := remote.readCalls[0].entryID
	failedID := remote.readCalls[1].entryID
	if len(pending) != 1 || pending[0].EntryID != failedID {
		t.Fatalf("pending after partial failure = %#v", pending)
	}
	first, err := store.entryState(ctx, "account", acknowledgedID)
	if err != nil {
		t.Fatal(err)
	}
	second, err := store.entryState(ctx, "account", failedID)
	if err != nil {
		t.Fatal(err)
	}
	if first.remoteStatus != "read" || second.remoteStatus != "unread" {
		t.Fatalf("remote baselines after partial failure = %#v %#v", first, second)
	}
}

func TestBlockedSyncAllowsLocalSnapshotReadAndStar(t *testing.T) {
	ctx := context.Background()
	store := openTestStore(t)
	if err := store.ApplySnapshot(ctx, "account", testSnapshot()); err != nil {
		t.Fatal(err)
	}
	entered := make(chan struct{})
	release := make(chan struct{})
	remote := &fakeRemote{
		snapshot:      testSnapshot(),
		browseEntered: entered,
		browseRelease: release,
	}
	service := NewService(store, remote, "account", false, nil)
	selection := model.Selection{Kind: model.SelectionAll, UnreadOnly: true}
	syncResult := make(chan error, 1)
	go func() {
		_, err := service.Sync(ctx, selection, 1, 2)
		syncResult <- err
	}()
	select {
	case <-entered:
	case <-time.After(2 * time.Second):
		t.Fatal("blocked sync did not enter in time")
	}

	if snapshot, err := service.LocalSnapshot(ctx, selection); err != nil || len(snapshot.Entries) != 2 {
		t.Fatalf("local snapshot during sync: entries=%d error=%v", len(snapshot.Entries), err)
	}
	if snapshot, _, err := service.MarkRead(ctx, selection, []int64{1}, nil, true, true); err != nil || snapshot.Entries[0].Status != "read" {
		t.Fatalf("read during sync: snapshot=%#v error=%v", snapshot, err)
	}
	if snapshot, err := service.SetStarred(ctx, selection, 2, true, nil); err != nil || len(snapshot.Entries) != 1 || !snapshot.Entries[0].Starred {
		t.Fatalf("star during sync: snapshot=%#v error=%v", snapshot, err)
	}

	close(release)
	select {
	case err := <-syncResult:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("blocked sync did not finish in time")
	}
	if err := service.Flush(ctx); err != nil {
		t.Fatal(err)
	}
}

type storedEntryState struct {
	remoteStatus string
}

func (store *Store) entryState(ctx context.Context, accountID string, entryID int64) (storedEntryState, error) {
	var state storedEntryState
	err := store.db.QueryRowContext(ctx, `SELECT remote_status FROM entries WHERE account_id=? AND id=?`, accountID, entryID).Scan(&state.remoteStatus)
	return state, err
}
