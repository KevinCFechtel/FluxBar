package miniflux

import (
	"bytes"
	"context"
	"encoding/base64"
	"image"
	"image/color"
	"image/png"
	"log"
	"os"
	"path/filepath"
	"strings"
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
			{ID: 10, Title: "One", URL: "https://one.example/articles/1", Content: "<p>First &amp; <strong>second</strong></p><img src=\"/hero.jpg\"><script>ignored</script>", FeedID: 7, Feed: &miniflux.Feed{ID: 7, Title: "Feed"}},
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
		if entries[0].Preview != "First & second" {
			t.Fatalf("preview = %q", entries[0].Preview)
		}
		if entries[0].ImageURL != "https://one.example/hero.jpg" {
			t.Fatalf("image URL = %q", entries[0].ImageURL)
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

func TestFeedIDSet(t *testing.T) {
	got := feedIDSet("114, 113,invalid,-1,0")
	if len(got) != 2 || !got[114] || !got[113] {
		t.Fatalf("feedIDSet() = %#v", got)
	}
}

func TestIconDiagnosticLogsMetadataAndDumpsFailedIcon(t *testing.T) {
	dumpDirectory := t.TempDir()
	t.Setenv("FLUXBAR_DEBUG_ICONS", "1")
	t.Setenv("FLUXBAR_DUMP_FAILED_ICONS", "1")
	t.Setenv("FLUXBAR_ICON_DUMP_DIR", dumpDirectory)
	rawIcon := []byte("0000ftypavifunsupported")
	dataURL := "data:image/avif;base64," + base64.StdEncoding.EncodeToString(rawIcon)
	client := &fakeClient{
		entries: &miniflux.EntryResultSet{Total: 1, Entries: miniflux.Entries{
			{ID: 10, FeedID: 7, Feed: &miniflux.Feed{ID: 7, Title: "Feed\nName"}},
		}},
		icons: map[int64]*miniflux.FeedIcon{7: {
			ID: 99, MimeType: "image/avif", Data: dataURL,
		}},
		iconCalls: make(map[int64]int),
	}
	var logs bytes.Buffer
	service := NewWithClient(client, log.New(&logs, "", 0))
	if _, _, err := service.Unread(context.Background()); err != nil {
		t.Fatal(err)
	}

	for _, expected := range []string{
		"level=error component=icon event=failed",
		"feed_id=7",
		`feed="Feed Name"`,
		"stage=decode",
		"api_icon_id=99",
		`api_mime="image/avif"`,
		`declared_mime="image/avif"`,
		`detected_mime="image/avif"`,
		"decoded_bytes=23",
		"fingerprint=",
		"level=info component=icon event=dumped",
		"level=debug component=icons event=summary feeds=1 success=0 failed=1 cached=0",
	} {
		if !strings.Contains(logs.String(), expected) {
			t.Errorf("log does not contain %q:\n%s", expected, logs.String())
		}
	}
	if strings.Contains(logs.String(), base64.StdEncoding.EncodeToString(rawIcon)) {
		t.Fatal("log contains the base64 image payload")
	}

	files, err := filepath.Glob(filepath.Join(dumpDirectory, "feed-7-*.avif"))
	if err != nil || len(files) != 1 {
		t.Fatalf("dump files = %v, error = %v", files, err)
	}
	dumped, err := os.ReadFile(files[0])
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(dumped, rawIcon) {
		t.Fatalf("dumped data = %q", dumped)
	}
	info, err := os.Stat(files[0])
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0o600 {
		t.Fatalf("dump permissions = %o", info.Mode().Perm())
	}
}

func TestUnreadCreatesAndLogsDarkModeVariant(t *testing.T) {
	t.Setenv("FLUXBAR_DEBUG_ICONS", "1")
	t.Setenv("FLUXBAR_DUMP_FAILED_ICONS", "")
	t.Setenv("FLUXBAR_ICON_BACKGROUND_ALWAYS", "")
	t.Setenv("FLUXBAR_ICON_BACKGROUND_NEVER", "")
	client := &fakeClient{
		entries: &miniflux.EntryResultSet{Total: 1, Entries: miniflux.Entries{
			{ID: 10, FeedID: 7, Feed: &miniflux.Feed{ID: 7, Title: "Dark Feed"}},
		}},
		icons:     map[int64]*miniflux.FeedIcon{7: {Data: darkPNGDataURL(t)}},
		iconCalls: make(map[int64]int),
	}
	var logs bytes.Buffer
	service := NewWithClient(client, log.New(&logs, "", 0))
	entries, _, err := service.Unread(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || len(entries[0].Icon) == 0 || len(entries[0].DarkIcon) == 0 {
		t.Fatalf("missing icon variants: %#v", entries)
	}
	for _, expected := range []string{
		"has_transparency=true",
		"classified_dark=true",
		"background_mode=auto",
		"background_added=true",
		"mean_luminance=0.002",
	} {
		if !strings.Contains(logs.String(), expected) {
			t.Errorf("log does not contain %q:\n%s", expected, logs.String())
		}
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

func darkPNGDataURL(t *testing.T) string {
	t.Helper()
	var output bytes.Buffer
	source := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	for y := 6; y < 26; y++ {
		for x := 6; x < 26; x++ {
			source.SetNRGBA(x, y, color.NRGBA{R: 8, G: 8, B: 8, A: 255})
		}
	}
	if err := png.Encode(&output, source); err != nil {
		t.Fatal(err)
	}
	return "data:image/png;base64," + base64.StdEncoding.EncodeToString(output.Bytes())
}
