package model

// Entry is the presentation-neutral representation of an unread Miniflux entry.
type Entry struct {
	ID       int64
	Title    string
	URL      string
	FeedID   int64
	FeedName string
	Preview  string
	ImageURL string
	Icon     []byte
	DarkIcon []byte
}
