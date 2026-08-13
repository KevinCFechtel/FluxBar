package miniflux

import (
	"context"
	"fmt"
	"log"
	"sync"

	"github.com/KevinCFechtel/FluxBar/internal/icons"
	"github.com/KevinCFechtel/FluxBar/internal/model"
	miniflux "miniflux.app/v2/client"
)

const iconWorkers = 6

type client interface {
	EntriesContext(context.Context, *miniflux.Filter) (*miniflux.EntryResultSet, error)
	UpdateEntriesContext(context.Context, []int64, string) error
	FeedIconContext(context.Context, int64) (*miniflux.FeedIcon, error)
}

type logger interface {
	Printf(string, ...any)
}

// Service owns the presentation-independent Miniflux operations and icon cache.
type Service struct {
	client client
	logger logger

	iconMu    sync.RWMutex
	iconCache map[int64][]byte
}

func New(server, apiKey string, logger *log.Logger) *Service {
	return NewWithClient(miniflux.NewClient(server, apiKey), logger)
}

func NewWithClient(client client, logger logger) *Service {
	if logger == nil {
		logger = log.Default()
	}
	return &Service{
		client:    client,
		logger:    logger,
		iconCache: make(map[int64][]byte),
	}
}

// Unread returns all unread entries ordered from oldest to newest.
func (service *Service) Unread(ctx context.Context) ([]model.Entry, int, error) {
	result, err := service.client.EntriesContext(ctx, &miniflux.Filter{
		Status:    miniflux.EntryStatusUnread,
		Order:     "published_at",
		Direction: "asc",
	})
	if err != nil {
		return nil, 0, fmt.Errorf("ungelesene Miniflux-Einträge laden: %w", err)
	}
	if result == nil {
		return nil, 0, fmt.Errorf("ungelesene Miniflux-Einträge laden: leere Antwort")
	}

	entries := make([]model.Entry, 0, len(result.Entries))
	feedEntries := make(map[int64][]int)
	for _, source := range result.Entries {
		if source == nil {
			continue
		}
		feedID := source.FeedID
		feedName := ""
		if source.Feed != nil {
			feedName = source.Feed.Title
			if feedID == 0 {
				feedID = source.Feed.ID
			}
		}
		entries = append(entries, model.Entry{
			ID:       source.ID,
			Title:    source.Title,
			URL:      source.URL,
			FeedID:   feedID,
			FeedName: feedName,
		})
		if feedID > 0 {
			feedEntries[feedID] = append(feedEntries[feedID], len(entries)-1)
		}
	}

	service.loadIcons(ctx, entries, feedEntries)
	return entries, result.Total, nil
}

func (service *Service) loadIcons(ctx context.Context, entries []model.Entry, feedEntries map[int64][]int) {
	if len(feedEntries) == 0 {
		return
	}
	type result struct {
		feedID int64
		icon   []byte
	}

	jobs := make(chan int64)
	results := make(chan result)
	workerCount := min(iconWorkers, len(feedEntries))
	var workers sync.WaitGroup
	for range workerCount {
		workers.Add(1)
		go func() {
			defer workers.Done()
			for feedID := range jobs {
				results <- result{feedID: feedID, icon: service.icon(ctx, feedID)}
			}
		}()
	}

	go func() {
		for feedID := range feedEntries {
			jobs <- feedID
		}
		close(jobs)
		workers.Wait()
		close(results)
	}()

	for loaded := range results {
		for _, index := range feedEntries[loaded.feedID] {
			entries[index].Icon = loaded.icon
		}
	}
}

func (service *Service) icon(ctx context.Context, feedID int64) []byte {
	service.iconMu.RLock()
	icon, known := service.iconCache[feedID]
	service.iconMu.RUnlock()
	if known {
		return icon
	}

	feedIcon, err := service.client.FeedIconContext(ctx, feedID)
	if err != nil {
		service.logger.Printf("Icon für Feed %d konnte nicht geladen werden: %v", feedID, err)
		return nil
	}
	if feedIcon == nil {
		service.logger.Printf("Miniflux lieferte für Feed %d kein Icon", feedID)
		return nil
	}
	icon, err = icons.NormalizeDataURL(feedIcon.Data, icons.DefaultSize)
	if err != nil {
		service.logger.Printf("Icon für Feed %d konnte nicht verarbeitet werden: %v", feedID, err)
		return nil
	}
	service.storeIcon(feedID, icon)
	return icon
}

func (service *Service) storeIcon(feedID int64, icon []byte) {
	service.iconMu.Lock()
	service.iconCache[feedID] = icon
	service.iconMu.Unlock()
}

func (service *Service) MarkRead(ctx context.Context, entryIDs ...int64) error {
	if len(entryIDs) == 0 {
		return nil
	}
	if err := service.client.UpdateEntriesContext(ctx, entryIDs, miniflux.EntryStatusRead); err != nil {
		return fmt.Errorf("Miniflux-Eintrag als gelesen markieren: %w", err)
	}
	return nil
}
