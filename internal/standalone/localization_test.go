package standalone

import (
	"strconv"
	"strings"
	"testing"
)

func TestTranslationsLoad(t *testing.T) {
	if translationLoadError != nil {
		t.Fatalf("load translations: %v", translationLoadError)
	}
}

func TestUnreadTooltipRendersCount(t *testing.T) {
	for _, count := range []int{1, 2} {
		if tooltip := unreadTooltip(count); !strings.Contains(tooltip, strconv.Itoa(count)) {
			t.Fatalf("unreadTooltip(%d) = %q", count, tooltip)
		}
	}
}
