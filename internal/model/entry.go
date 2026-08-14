package model

import "time"

// Entry is the presentation-neutral representation of an unread Miniflux entry.
type Entry struct {
	ID          int64
	Title       string
	URL         string
	FeedID      int64
	FeedName    string
	PublishedAt time.Time
	Preview     string
	ImageURL    string
	Icon        []byte
	DarkIcon    []byte
}
