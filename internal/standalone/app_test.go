package standalone

import (
	"context"
	"io"
	"log"
	"strings"
	"testing"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type actionReader struct {
	marked []int64
}

func (reader *actionReader) Unread(context.Context) ([]model.Entry, int, error) {
	return nil, 0, nil
}

func (reader *actionReader) MarkRead(_ context.Context, ids ...int64) error {
	reader.marked = append(reader.marked, ids...)
	return nil
}

func TestMenuLabel(t *testing.T) {
	got := menuLabel(model.Entry{FeedName: "Feed", Title: "Article"})
	if got != "Feed: Article" {
		t.Fatalf("menuLabel() = %q", got)
	}
}

func TestMenuLabelIsTruncated(t *testing.T) {
	got := menuLabel(model.Entry{FeedName: "Feed", Title: strings.Repeat("x", 200)})
	if len([]rune(got)) != 120 || !strings.HasSuffix(got, "…") {
		t.Fatalf("menuLabel() has %d runes and value %q", len([]rune(got)), got)
	}
}

func TestIconForAppearance(t *testing.T) {
	entry := model.Entry{Icon: []byte("regular"), DarkIcon: []byte("dark")}
	if got := string(iconForAppearance(entry, false)); got != "regular" {
		t.Fatalf("light icon = %q", got)
	}
	if got := string(iconForAppearance(entry, true)); got != "dark" {
		t.Fatalf("dark icon = %q", got)
	}
	entry.DarkIcon = nil
	if got := string(iconForAppearance(entry, true)); got != "regular" {
		t.Fatalf("dark fallback icon = %q", got)
	}
}

func TestNotifyAppearanceCoalescesLatestValueWithoutRefresh(t *testing.T) {
	app := New(&actionReader{}, log.New(io.Discard, "", 0), nil)

	app.notifyAppearance(false)
	app.notifyAppearance(true)

	if got := len(app.appearanceSignal); got != 1 {
		t.Fatalf("appearance signals = %d, want 1", got)
	}
	if !app.appearanceDark.Load() {
		t.Fatal("appearance value is not the latest value")
	}
	if got := len(app.refresh); got != 0 {
		t.Fatalf("refresh requests = %d, want 0", got)
	}
}

func TestOpenAndMarkReadUsesBrowser(t *testing.T) {
	previousOpenBrowser := openBrowser
	defer func() { openBrowser = previousOpenBrowser }()
	var opened string
	openBrowser = func(target string) error {
		opened = target
		return nil
	}

	reader := &actionReader{}
	app := New(reader, log.New(io.Discard, "", 0), nil)
	entry := model.Entry{ID: 42, URL: "https://example.com/article"}
	app.openAndMarkRead(reader, entry)

	if opened != entry.URL || len(reader.marked) != 1 || reader.marked[0] != entry.ID {
		t.Fatalf("opened=%q marked=%v", opened, reader.marked)
	}
}
