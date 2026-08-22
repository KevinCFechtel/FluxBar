package model

const (
	SelectionAll      = "all"
	SelectionUnread   = "unread"
	SelectionStarred  = "starred"
	SelectionCategory = "category"
	SelectionFeed     = "feed"
)

type Selection struct {
	Kind       string `json:"kind"`
	ID         int64  `json:"id,omitempty"`
	UnreadOnly bool   `json:"unreadOnly,omitempty"`
}

func (selection Selection) Normalized() Selection {
	switch selection.Kind {
	case SelectionAll:
		return selection
	case SelectionUnread, SelectionStarred:
		return Selection{Kind: selection.Kind, UnreadOnly: selection.UnreadOnly}
	case SelectionCategory, SelectionFeed:
		if selection.ID > 0 {
			return selection
		}
	}
	return Selection{Kind: SelectionAll, UnreadOnly: true}
}

type Feed struct {
	ID          int64  `json:"id"`
	Title       string `json:"title"`
	CategoryID  int64  `json:"categoryID"`
	UnreadCount int    `json:"unreadCount"`
}

type Category struct {
	ID          int64  `json:"id"`
	Title       string `json:"title"`
	UnreadCount int    `json:"unreadCount"`
	Feeds       []Feed `json:"feeds"`
}

type BrowseSnapshot struct {
	Version      int        `json:"version"`
	Selection    Selection  `json:"selection"`
	Entries      []Entry    `json:"entries"`
	Categories   []Category `json:"categories"`
	Total        int        `json:"total"`
	UnreadTotal  int        `json:"unreadTotal"`
	StarredTotal int        `json:"starredTotal"`
}
