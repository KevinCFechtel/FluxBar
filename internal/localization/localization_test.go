package localization

import "testing"

func TestRepresentativeLocalesAndFallbacks(t *testing.T) {
	tests := []struct {
		name     string
		locale   string
		key      string
		fallback string
		expected string
	}{
		{name: "English", locale: "en-US", key: "menu.refresh", fallback: "fallback", expected: "Refresh"},
		{name: "German", locale: "de-DE", key: "menu.refresh", fallback: "fallback", expected: "Aktualisieren"},
		{name: "unsupported locale uses English catalog", locale: "fr-FR", key: "menu.refresh", fallback: "fallback", expected: "Refresh"},
		{name: "unknown key uses caller fallback", locale: "de-DE", key: "missing.key", fallback: "Fallback", expected: "Fallback"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			localizer, err := New(test.locale)
			if err != nil {
				t.Fatalf("New(%q): %v", test.locale, err)
			}
			if actual := localizer.Text(test.key, test.fallback); actual != test.expected {
				t.Fatalf("Text(%q, %q) = %q, want %q", test.key, test.fallback, actual, test.expected)
			}
		})
	}
}

func TestNoLocalePreferenceUsesEnglish(t *testing.T) {
	localizer, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	if actual := localizer.Text("menu.refresh", "fallback"); actual != "Refresh" {
		t.Fatalf("Text = %q, want %q", actual, "Refresh")
	}
}

func TestParameterizedMessage(t *testing.T) {
	localizer, err := New("de")
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	if actual := localizer.Format("status.error_format", "Error: %s", "kaputt"); actual != "Fehler: kaputt" {
		t.Fatalf("Format = %q, want %q", actual, "Fehler: kaputt")
	}
}

func TestPluralization(t *testing.T) {
	localizer, err := New("de")
	if err != nil {
		t.Fatalf("New: %v", err)
	}

	tests := []struct {
		count    int
		expected string
	}{
		{count: 1, expected: "FluxNews — 1 ungelesener Artikel"},
		{count: 2, expected: "FluxNews — 2 ungelesene Artikel"},
	}
	for _, test := range tests {
		actual := localizer.Plural(
			"status.unread_count",
			"FluxNews — {{.Count}} unread article",
			"FluxNews — {{.Count}} unread articles",
			test.count,
			map[string]any{"Count": test.count},
		)
		if actual != test.expected {
			t.Fatalf("Plural(%d) = %q, want %q", test.count, actual, test.expected)
		}
	}

	actual := localizer.Plural(
		"missing.plural",
		"{{.Count}} item",
		"{{.Count}} items",
		2,
		map[string]any{"Count": 2},
	)
	if actual != "2 items" {
		t.Fatalf("fallback plural = %q, want %q", actual, "2 items")
	}
}
