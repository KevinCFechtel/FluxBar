package miniflux

import (
	"bytes"
	"context"
	"encoding/base64"
	"image"
	"image/color"
	"image/png"
	"sync"
	"testing"

	miniflux "miniflux.app/v2/client"
)

type fakeClient struct {
	entries *miniflux.EntryResultSet
	icons   map[int64]*miniflux.FeedIcon

	mu          sync.Mutex
	iconCalls   map[int64]int
	updatedIDs  []int64
	updatedWith string
}

func (client *fakeClient) EntriesContext(context.Context, *miniflux.Filter) (*miniflux.EntryResultSet, error) {
	return client.entries, nil
}

func (client *fakeClient) FeedIconContext(_ context.Context, feedID int64) (*miniflux.FeedIcon, error) {
	client.mu.Lock()
	client.iconCalls[feedID]++
	client.mu.Unlock()
	return client.icons[feedID], nil
}

func (client *fakeClient) UpdateEntriesContext(_ context.Context, ids []int64, status string) error {
	client.updatedIDs = append([]int64(nil), ids...)
	client.updatedWith = status
	return nil
}

func TestUnreadLoadsEachFeedIconOnceAndCachesIt(t *testing.T) {
	client := &fakeClient{
		entries: &miniflux.EntryResultSet{Total: 2, Entries: miniflux.Entries{
			{ID: 10, Title: "One", URL: "https://one.example", FeedID: 7, Feed: &miniflux.Feed{ID: 7, Title: "Feed"}},
			{ID: 11, Title: "Two", URL: "https://two.example", FeedID: 7, Feed: &miniflux.Feed{ID: 7, Title: "Feed"}},
		}},
		icons:     map[int64]*miniflux.FeedIcon{7: {Data: pngDataURL(t)}},
		iconCalls: make(map[int64]int),
	}
	service := NewWithClient(client, nil)

	for range 2 {
		entries, total, err := service.Unread(context.Background())
		if err != nil {
			t.Fatal(err)
		}
		if total != 2 || len(entries) != 2 || len(entries[0].Icon) == 0 || len(entries[1].Icon) == 0 {
			t.Fatalf("unexpected unread result: total=%d entries=%#v", total, entries)
		}
	}
	if client.iconCalls[7] != 1 {
		t.Fatalf("FeedIconContext called %d times", client.iconCalls[7])
	}
}

func TestMarkRead(t *testing.T) {
	client := &fakeClient{iconCalls: make(map[int64]int)}
	service := NewWithClient(client, nil)
	if err := service.MarkRead(context.Background(), 3, 4); err != nil {
		t.Fatal(err)
	}
	if len(client.updatedIDs) != 2 || client.updatedIDs[0] != 3 || client.updatedWith != miniflux.EntryStatusRead {
		t.Fatalf("unexpected update: %#v %q", client.updatedIDs, client.updatedWith)
	}
}

func pngDataURL(t *testing.T) string {
	t.Helper()
	var output bytes.Buffer
	source := image.NewRGBA(image.Rect(0, 0, 1, 1))
	source.Set(0, 0, color.RGBA{R: 255, A: 255})
	if err := png.Encode(&output, source); err != nil {
		t.Fatal(err)
	}
	return "data:image/png;base64," + base64.StdEncoding.EncodeToString(output.Bytes())
}
