// sqlite-compat is a test-only helper invoked by Build/test-sqlite-compat.sh.
package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/inbox"
	"github.com/KevinCFechtel/FluxBar/internal/miniflux"
	"github.com/KevinCFechtel/FluxBar/internal/model"
)

func remoteBrowse(baseURL, apiKey, kind, rawID string, unreadOnly bool) {
	id, err := strconv.ParseInt(rawID, 10, 64)
	if err != nil {
		panic(err)
	}
	service := miniflux.NewWithSortOrder(baseURL, apiKey, miniflux.SortOldestFirst, nil)
	snapshot, err := service.Browse(context.Background(), model.Selection{
		Kind: kind, ID: id, UnreadOnly: unreadOnly,
	})
	if err != nil {
		fmt.Fprintln(os.Stderr, "go-error:", err)
		os.Exit(3)
	}
	encoded, err := json.Marshal(snapshot)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(encoded))
}

const (
	server = "https://compat.example"
	apiKey = "compat-key"
)

func main() {
	var path string
	if len(os.Args) < 3 {
		panic("usage: sqlite-compat <mode> <database> [args...]")
	}
	if os.Args[1] != "remote-browse" {
		var pathErr error
		path, pathErr = filepath.Abs(os.Args[2])
		if pathErr != nil {
			panic(pathErr)
		}
		requireTemporaryPath(path)
	}

	switch os.Args[1] {
	case "create":
		create(path)
	case "read-rust":
		readRust(path)
	case "create-mutations":
		createMutations(path)
	case "continue-rust-mutations":
		continueMutations(path, "Rust")
	case "fixture-basic", "fixture-large", "fixture-multi", "fixture-empty":
		buildFixture(os.Args[1], path)
	case "remote-browse":
		// remote-browse <baseURL> <apiKey> <kind> <id> <unreadOnly>
		if len(os.Args) != 7 {
			panic("remote-browse requires baseURL, apiKey, kind, id, unreadOnly")
		}
		remoteBrowse(os.Args[2], os.Args[3], os.Args[4], os.Args[5], os.Args[6] == "true")
	case "snapshot":
		// snapshot <db> <kind> <id> <unreadOnly> <retainCSV>
		if len(os.Args) != 7 {
			panic("snapshot requires kind, id, unreadOnly, retainCSV")
		}
		rawID, parseErr := strconv.ParseInt(os.Args[4], 10, 64)
		if parseErr != nil {
			panic(parseErr)
		}
		snapshot(path, os.Args[3], rawID, os.Args[5] == "true", os.Args[6])
	case "sync-probe":
		// sync-probe <db> <baseURL> <scenario>
		if len(os.Args) != 5 {
			panic("sync-probe requires baseURL and scenario")
		}
		syncProbe(path, os.Args[3], os.Args[4])
	default:
		panic("unsupported mode")
	}
}

func createMutations(path string) {
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	ctx := context.Background()
	account := inbox.AccountID(server, apiKey)
	if err := store.EnsureAccount(ctx, account, server); err != nil {
		panic(err)
	}
	if err := store.ApplySnapshot(ctx, account, testMutationSnapshot()); err != nil {
		panic(err)
	}
	if _, err := store.SetRead(ctx, account, []int64{1}, true, true); err != nil {
		panic(err)
	}
	if err := store.SetStarred(ctx, account, 2, true); err != nil {
		panic(err)
	}
}

func continueMutations(path, producer string) {
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	ctx := context.Background()
	account := inbox.AccountID(server, apiKey)
	pending, err := store.Pending(ctx, account)
	if err != nil || len(pending) != 2 {
		panic(fmt.Sprintf("%s pending=%#v error=%v", producer, pending, err))
	}
	db, err := sqlOpenHelper(path)
	if err != nil {
		panic(err)
	}
	var batchID string
	if err := db.QueryRow(`SELECT id FROM undo_batches WHERE account_id=?`, account).Scan(&batchID); err != nil {
		panic(err)
	}
	db.Close()
	if _, err := store.Undo(ctx, account, batchID); err != nil {
		panic(err)
	}
	pending, err = store.Pending(ctx, account)
	if err != nil || len(pending) != 2 {
		panic(fmt.Sprintf("%s continued pending=%#v error=%v", producer, pending, err))
	}
}

func testMutationSnapshot() model.BrowseSnapshot {
	return model.BrowseSnapshot{
		Version: 1, Selection: model.Selection{Kind: model.SelectionAll, UnreadOnly: true},
		Entries: []model.Entry{
			{ID: 1, Title: "One", URL: "https://example.com/1", FeedID: 20, FeedName: "Feed", CategoryID: 10, PublishedAt: time.Date(2026, 8, 22, 10, 0, 1, 0, time.UTC), Status: "unread"},
			{ID: 2, Title: "Two", URL: "https://example.com/2", FeedID: 20, FeedName: "Feed", CategoryID: 10, PublishedAt: time.Date(2026, 8, 22, 10, 0, 2, 0, time.UTC), Status: "unread"},
		},
		Categories: []model.Category{{ID: 10, Title: "Category", UnreadCount: 2, Feeds: []model.Feed{{ID: 20, Title: "Feed", CategoryID: 10, UnreadCount: 2}}}},
		Total:      2, UnreadTotal: 2,
	}
}

func syncProbe(path, baseURL, scenario string) {
	ctx := context.Background()
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	account := inbox.AccountID(baseURL, apiKey)
	if err := store.EnsureAccount(ctx, account, baseURL); err != nil {
		panic(err)
	}
	service := inbox.NewService(store, miniflux.New(baseURL, apiKey, nil), account, false, nil)
	selection := model.Selection{Kind: model.SelectionAll, UnreadOnly: true}
	if scenario == "incremental" {
		selection.UnreadOnly = false
	}
	result, operationErr := service.Sync(ctx, selection)
	if operationErr == nil {
		switch scenario {
		case "initial":
		case "incremental", "incomplete", "refresh-5xx", "refresh-auth":
			result, operationErr = service.Sync(ctx, selection)
		case "read":
			_, _ = store.SetRead(ctx, account, []int64{1}, true, false)
			operationErr = service.Flush(ctx)
		case "read-reversal":
			_, _ = store.SetRead(ctx, account, []int64{1}, true, false)
			_, _ = store.SetRead(ctx, account, []int64{1}, false, false)
			operationErr = service.Flush(ctx)
		case "star-reversal":
			_ = store.SetStarred(ctx, account, 1, true)
			_ = store.SetStarred(ctx, account, 1, false)
			operationErr = service.Flush(ctx)
		case "star":
			_ = store.SetStarred(ctx, account, 1, true)
			operationErr = service.Flush(ctx)
		case "pending-stale":
			_, _ = store.SetRead(ctx, account, []int64{1}, true, false)
			_ = store.SetStarred(ctx, account, 1, true)
			result, operationErr = service.Sync(ctx, selection)
		case "partial-failure", "full-failure":
			_, _ = store.SetRead(ctx, account, []int64{1, 2}, true, false)
			operationErr = service.Flush(ctx)
		case "undo-after-flush":
			receipt, mutationErr := store.SetRead(ctx, account, []int64{1}, true, true)
			if mutationErr != nil {
				operationErr = mutationErr
			} else if operationErr = service.Flush(ctx); operationErr == nil {
				_, operationErr = store.Undo(ctx, account, receipt.ID)
				if operationErr == nil {
					operationErr = service.Flush(ctx)
				}
			}
		case "undo-before-flush":
			receipt, mutationErr := store.SetRead(ctx, account, []int64{1}, true, true)
			if mutationErr != nil {
				operationErr = mutationErr
			} else if _, operationErr = store.Undo(ctx, account, receipt.ID); operationErr == nil {
				operationErr = service.Flush(ctx)
			}
		case "discard-undo":
			receipt, mutationErr := store.SetRead(ctx, account, []int64{1}, true, true)
			if mutationErr != nil {
				operationErr = mutationErr
			} else if operationErr = store.DiscardUndo(ctx, account, receipt.ID); operationErr == nil {
				operationErr = service.Flush(ctx)
			}
		default:
			panic("unknown sync scenario")
		}
		if local, localErr := service.LocalSnapshot(ctx, selection); localErr == nil {
			result = local
		} else if operationErr == nil {
			operationErr = localErr
		}
	}
	output := struct {
		Error    string               `json:"error,omitempty"`
		Snapshot model.BrowseSnapshot `json:"snapshot"`
	}{Snapshot: result}
	if operationErr != nil {
		output.Error = operationErr.Error()
	}
	encoded, err := json.Marshal(output)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(encoded))
}

func create(path string) {
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	account := inbox.AccountID(server, apiKey)
	ctx := context.Background()
	if err := store.EnsureAccount(ctx, account, server); err != nil {
		panic(err)
	}
	if err := store.ApplySnapshot(ctx, account, model.BrowseSnapshot{
		Version:   1,
		Selection: model.Selection{Kind: model.SelectionAll},
		Entries: []model.Entry{{
			ID: 30, Title: "Go Entry", URL: "https://example.com/go",
			FeedID: 20, FeedName: "Go Feed", CategoryID: 10,
			PublishedAt: time.Date(2026, 8, 22, 12, 34, 56, 123456789, time.UTC),
			Preview:     "Go preview", Status: "go-future-status", Starred: true,
		}},
		Categories: []model.Category{{
			ID: 10, Title: "Go Category", UnreadCount: 4,
			Feeds: []model.Feed{{ID: 20, Title: "Go Feed", CategoryID: 10, UnreadCount: 4}},
		}},
		Total: 1,
	}); err != nil {
		panic(err)
	}
}

func readRust(path string) {
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	account := inbox.AccountID(server, apiKey)
	snapshot, err := store.Snapshot(context.Background(), account, model.Selection{Kind: model.SelectionAll}, false, nil)
	if err != nil {
		panic(err)
	}
	if len(snapshot.Entries) != 1 {
		panic(fmt.Sprintf("entries=%d, want 1", len(snapshot.Entries)))
	}
	entry := snapshot.Entries[0]
	if entry.Title != "Rust Entry" || entry.Status != "rust-future-status" || !entry.Starred {
		panic(fmt.Sprintf("unexpected Rust entry: %#v", entry))
	}
	if entry.PublishedAt.Format(time.RFC3339Nano) != "2026-08-22T12:34:56.123456789Z" {
		panic(fmt.Sprintf("unexpected timestamp: %s", entry.PublishedAt.Format(time.RFC3339Nano)))
	}
}

func requireTemporaryPath(path string) {
	temporary, err := filepath.EvalSymlinks(os.TempDir())
	if err != nil {
		panic(err)
	}
	parent, err := filepath.EvalSymlinks(filepath.Dir(path))
	if err != nil {
		panic(err)
	}
	relative, err := filepath.Rel(temporary, parent)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		panic("refusing non-temporary database path")
	}
}

// fixtureAccount is the second account used for isolation fixtures.
const (
	fixtureServer      = "https://fixture.example"
	fixtureKey         = "fixture-key"
	fixtureOtherServer = "https://other.example"
)

func buildFixture(mode, path string) {
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	ctx := context.Background()
	account := inbox.AccountID(fixtureServer, fixtureKey)

	if err := store.EnsureAccount(ctx, account, fixtureServer); err != nil {
		panic(err)
	}

	if mode == "fixture-empty" {
		return
	}

	if mode == "fixture-multi" {
		other := inbox.AccountID(fixtureOtherServer, fixtureKey)
		if err := store.EnsureAccount(ctx, other, fixtureOtherServer); err != nil {
			panic(err)
		}
	}

	db, err := sqlOpenHelper(path)
	if err != nil {
		panic(err)
	}
	defer db.Close()
	if _, err = db.Exec(`INSERT INTO categories(account_id,id,title) VALUES(?,?,?)`,
		account, 10, "Tech"); err != nil {
		panic(err)
	}
	if _, err = db.Exec(`INSERT INTO categories(account_id,id,title) VALUES(?,?,?)`,
		account, 20, "news"); err != nil {
		panic(err)
	}
	for _, feed := range []struct {
		id    int64
		title string
		cat   int64
		count int64
	}{
		{100, "Alpha Feed", 10, 2},
		{101, "alpha feed", 10, 1},
		{200, "Daily", 20, 3},
	} {
		if _, err = db.Exec(
			`INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count) VALUES(?,?,?,?,?)`,
			account, feed.id, feed.cat, feed.title, feed.count); err != nil {
			panic(err)
		}
	}
	// Orphan feed: category id 99 does not exist.
	if _, err = db.Exec(
		`INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count) VALUES(?,?,?,?,?)`,
		account, 300, 99, "Orphan", 7); err != nil {
		panic(err)
	}

	insertEntry := func(id int64, title, status string, starred bool, published time.Time) {
		remoteStatus := status
		_, err = db.Exec(`INSERT INTO entries(account_id,id,title,url,comments_url,feed_id,feed_name,
category_id,published_at,preview,image_url,remote_status,remote_starred,status,starred)
VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)`,
			account, id, title, fmt.Sprintf("https://example.com/%d", id), "", int64(100),
			"Alpha Feed", int64(10), published.UTC().Format(time.RFC3339Nano),
			fmt.Sprintf("Preview %d", id), "", remoteStatus, false, status, starred)
		if err != nil {
			panic(err)
		}
	}
	base := time.Date(2026, 8, 22, 10, 0, 0, 0, time.UTC)
	insertEntry(1, "Unread One", "unread", false, base)
	insertEntry(2, "Read Starred", "read", true, base.Add(time.Second))
	insertEntry(3, "Unknown Status", "legacy-status", true, base.Add(2*time.Second))
	insertEntry(4, "Unread Two", "unread", false, base.Add(3*time.Second))

	if mode == "fixture-large" {
		for index := int64(5); index < 205; index++ {
			insertEntry(index, fmt.Sprintf("Bulk %03d", index), "unread", false,
				base.Add(time.Duration(index)*time.Second))
		}
	}

	// Remote selection total larger than the local row count plus a pending
	// read divergence on entry 4 (locally unread, remotely read).
	if _, err = db.Exec(`INSERT INTO selection_totals(account_id,kind,selection_id,unread_only,total)
VALUES(?,?,?,?,?)`, account, "all", 0, false, 50); err != nil {
		panic(err)
	}
	if _, err = db.Exec(`UPDATE entries SET remote_status='read' WHERE account_id=? AND id=4`, account); err != nil {
		panic(err)
	}
	if _, err = db.Exec(`INSERT INTO pending_mutations(account_id,entry_id,field,desired,revision,updated_at)
VALUES(?,?,?,?,?,?)`, account, int64(4), "read", true, int64(1),
		base.Add(4*time.Second).UTC().Format(time.RFC3339Nano)); err != nil {
		panic(err)
	}
	if _, err = db.Exec(`UPDATE accounts SET remote_starred_total=9 WHERE id=?`, account); err != nil {
		panic(err)
	}

	if mode == "fixture-multi" {
		other := inbox.AccountID(fixtureOtherServer, fixtureKey)
		if _, err = db.Exec(`INSERT INTO categories(account_id,id,title) VALUES(?,?,?)`,
			other, 10, "Other Tech"); err != nil {
			panic(err)
		}
		if _, err = db.Exec(
			`INSERT INTO feeds(account_id,id,category_id,title,remote_unread_count) VALUES(?,?,?,?,?)`,
			other, int64(100), int64(10), "Other Feed", int64(5)); err != nil {
			panic(err)
		}
		_, err = db.Exec(`INSERT INTO entries(account_id,id,title,url,comments_url,feed_id,feed_name,
category_id,published_at,preview,image_url,remote_status,remote_starred,status,starred)
VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)`,
			other, int64(1), "Other Account Entry", "https://other.example/1", "", int64(100),
			"Other Feed", int64(10), base.UTC().Format(time.RFC3339Nano), "Other preview", "",
			"unread", false, "unread", false)
		if err != nil {
			panic(err)
		}
	}
}

// snapshot prints the Go local snapshot for the given selection as JSON.
func snapshot(path, kind string, rawID int64, unreadOnly bool, retainCSV string) {
	store, err := inbox.OpenStore(path)
	if err != nil {
		panic(err)
	}
	defer store.Close()
	var retainIDs []int64
	for _, field := range strings.Split(retainCSV, ",") {
		field = strings.TrimSpace(field)
		if field == "" {
			continue
		}
		value, parseErr := strconv.ParseInt(field, 10, 64)
		if parseErr != nil {
			panic(parseErr)
		}
		retainIDs = append(retainIDs, value)
	}
	account := inbox.AccountID(fixtureServer, fixtureKey)
	result, err := store.Snapshot(context.Background(), account,
		model.Selection{Kind: kind, ID: rawID, UnreadOnly: unreadOnly}, false, retainIDs)
	if err != nil {
		panic(err)
	}
	encoded, err := json.Marshal(result)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(encoded))
}

func sqlOpenHelper(path string) (*sql.DB, error) {
	return sql.Open("sqlite3", path+"?_busy_timeout=5000&_foreign_keys=on&_journal_mode=WAL&_synchronous=NORMAL")
}
