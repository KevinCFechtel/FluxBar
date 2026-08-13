package standalone

import (
	"strings"
	"testing"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

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
