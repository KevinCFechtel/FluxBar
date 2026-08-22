package model

import "testing"

// Characterization tests locking the observable Normalized() behavior for the
// Rust compatibility migration. The expected values were derived from the
// existing implementation and must not change during the language migration.
func TestSelectionNormalization(t *testing.T) {
	cases := []struct {
		name     string
		input    Selection
		expected Selection
	}{
		{"all keeps everything", Selection{Kind: SelectionAll, ID: 5, UnreadOnly: false}, Selection{Kind: SelectionAll, ID: 5, UnreadOnly: false}},
		{"unread drops id", Selection{Kind: SelectionUnread, ID: 5, UnreadOnly: false}, Selection{Kind: SelectionUnread, ID: 0, UnreadOnly: false}},
		{"starred drops id", Selection{Kind: SelectionStarred, ID: 9, UnreadOnly: true}, Selection{Kind: SelectionStarred, ID: 0, UnreadOnly: true}},
		{"category keeps valid id", Selection{Kind: SelectionCategory, ID: 7, UnreadOnly: true}, Selection{Kind: SelectionCategory, ID: 7, UnreadOnly: true}},
		{"feed keeps valid id", Selection{Kind: SelectionFeed, ID: 3, UnreadOnly: false}, Selection{Kind: SelectionFeed, ID: 3, UnreadOnly: false}},
		{"category zero id falls back", Selection{Kind: SelectionCategory, ID: 0, UnreadOnly: false}, Selection{Kind: SelectionAll, ID: 0, UnreadOnly: true}},
		{"feed negative id falls back", Selection{Kind: SelectionFeed, ID: -1, UnreadOnly: true}, Selection{Kind: SelectionAll, ID: 0, UnreadOnly: true}},
		{"empty kind falls back", Selection{}, Selection{Kind: SelectionAll, ID: 0, UnreadOnly: true}},
		{"unknown kind falls back", Selection{Kind: "bogus", ID: 4, UnreadOnly: false}, Selection{Kind: SelectionAll, ID: 0, UnreadOnly: true}},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			if got := testCase.input.Normalized(); got != testCase.expected {
				t.Fatalf("Normalized(%#v) = %#v, want %#v", testCase.input, got, testCase.expected)
			}
		})
	}
}
