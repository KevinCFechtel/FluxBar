package model

import "time"

// Entry is the presentation-neutral representation of a Miniflux entry.
type Entry struct {
	ID          int64     `json:"id"`
	Title       string    `json:"title"`
	URL         string    `json:"url"`
	CommentsURL string    `json:"commentsURL,omitempty"`
	FeedID      int64     `json:"feedID"`
	FeedName    string    `json:"feedName"`
	CategoryID  int64     `json:"categoryID,omitempty"`
	PublishedAt time.Time `json:"publishedAt"`
	Preview     string    `json:"preview"`
	ImageURL    string    `json:"imageURL,omitempty"`
	Status      string    `json:"status"`
	Starred     bool      `json:"starred"`
	Icon        []byte    `json:"icon,omitempty"`
	DarkIcon    []byte    `json:"darkIcon,omitempty"`
}
