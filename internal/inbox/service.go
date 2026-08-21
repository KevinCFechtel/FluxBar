package inbox

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"log"
	"sync"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type RemoteEntryState struct {
	Starred bool
}

type Remote interface {
	Browse(context.Context, model.Selection) (model.BrowseSnapshot, error)
	SetReadBatch(context.Context, []int64, bool) error
	EntryState(context.Context, int64) (RemoteEntryState, error)
	ToggleStarred(context.Context, int64) error
	FeedIcon(context.Context, int64, string) ([]byte, []byte)
}

type Service struct {
	store       *Store
	remote      Remote
	accountID   string
	newestFirst bool
	logger      *log.Logger
	syncMu      sync.Mutex
	timerMu     sync.Mutex
	flushTimer  *time.Timer
}

func AccountID(server, apiKey string) string {
	sum := sha256.Sum256([]byte(server + "\x00" + apiKey))
	return hex.EncodeToString(sum[:])
}

func NewService(store *Store, remote Remote, accountID string, newestFirst bool, logger *log.Logger) *Service {
	if logger == nil {
		logger = log.Default()
	}
	return &Service{store: store, remote: remote, accountID: accountID, newestFirst: newestFirst, logger: logger}
}

func (service *Service) LocalSnapshot(ctx context.Context, selection model.Selection, retainIDs ...int64) (model.BrowseSnapshot, error) {
	return service.store.Snapshot(ctx, service.accountID, selection, service.newestFirst, retainIDs)
}

func (service *Service) Sync(ctx context.Context, selection model.Selection, retainIDs ...int64) (model.BrowseSnapshot, error) {
	service.syncMu.Lock()
	defer service.syncMu.Unlock()
	if err := service.flushPending(ctx); err != nil {
		service.logger.Printf("level=warning component=sync event=pending_failed error=%q", err.Error())
	}
	remoteSnapshot, err := service.remote.Browse(ctx, selection)
	if err != nil {
		local, localErr := service.LocalSnapshot(ctx, selection, retainIDs...)
		if localErr != nil {
			return model.BrowseSnapshot{}, localErr
		}
		return local, err
	}
	if err := service.store.ApplySnapshot(ctx, service.accountID, remoteSnapshot); err != nil {
		return model.BrowseSnapshot{}, err
	}
	return service.LocalSnapshot(ctx, selection, retainIDs...)
}

func (service *Service) MarkRead(ctx context.Context, selection model.Selection, ids, retainIDs []int64, read, automatic bool) (model.BrowseSnapshot, *MutationReceipt, error) {
	receipt, err := service.store.SetRead(ctx, service.accountID, ids, read, automatic)
	if err != nil {
		return model.BrowseSnapshot{}, nil, err
	}
	snapshot, err := service.LocalSnapshot(ctx, selection, append(retainIDs, ids...)...)
	if err != nil {
		return model.BrowseSnapshot{}, nil, err
	}
	if automatic {
		service.ScheduleFlush(10 * time.Second)
	} else {
		service.ScheduleFlush(0)
	}
	return snapshot, receipt, nil
}

func (service *Service) SetStarred(ctx context.Context, selection model.Selection, id int64, starred bool, retainIDs []int64) (model.BrowseSnapshot, error) {
	if err := service.store.SetStarred(ctx, service.accountID, id, starred); err != nil {
		return model.BrowseSnapshot{}, err
	}
	snapshot, err := service.LocalSnapshot(ctx, selection, append(retainIDs, id)...)
	if err == nil {
		service.ScheduleFlush(0)
	}
	return snapshot, err
}

func (service *Service) Undo(ctx context.Context, selection model.Selection, receiptID string, retainIDs []int64) (model.BrowseSnapshot, error) {
	ids, err := service.store.Undo(ctx, service.accountID, receiptID)
	if err != nil {
		return model.BrowseSnapshot{}, err
	}
	snapshot, err := service.LocalSnapshot(ctx, selection, append(retainIDs, ids...)...)
	if err == nil {
		service.ScheduleFlush(0)
	}
	return snapshot, err
}

func (service *Service) DiscardUndo(ctx context.Context, receiptID string) error {
	return service.store.DiscardUndo(ctx, service.accountID, receiptID)
}

func (service *Service) Flush(ctx context.Context) error {
	service.syncMu.Lock()
	defer service.syncMu.Unlock()
	return service.flushPending(ctx)
}

func (service *Service) ScheduleFlush(delay time.Duration) {
	service.timerMu.Lock()
	defer service.timerMu.Unlock()
	if service.flushTimer != nil {
		service.flushTimer.Stop()
	}
	service.flushTimer = time.AfterFunc(delay, func() {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		if err := service.Flush(ctx); err != nil {
			service.logger.Printf("level=warning component=sync event=flush_failed error=%q", err.Error())
		}
	})
}

func (service *Service) flushPending(ctx context.Context) error {
	pending, err := service.store.Pending(ctx, service.accountID)
	if err != nil {
		return err
	}
	for _, mutation := range pending {
		switch mutation.Field {
		case "read":
			if err := service.remote.SetReadBatch(ctx, []int64{mutation.EntryID}, mutation.Desired); err != nil {
				return err
			}
		case "starred":
			state, err := service.remote.EntryState(ctx, mutation.EntryID)
			if err != nil {
				return err
			}
			if state.Starred != mutation.Desired {
				if err := service.remote.ToggleStarred(ctx, mutation.EntryID); err != nil {
					return err
				}
			}
		default:
			return fmt.Errorf("unbekanntes Mutationsfeld %q", mutation.Field)
		}
		if err := service.store.Acknowledge(ctx, service.accountID, mutation); err != nil {
			return err
		}
	}
	return nil
}

func (service *Service) FeedIcon(ctx context.Context, feedID int64, feedName string) ([]byte, []byte) {
	return service.remote.FeedIcon(ctx, feedID, feedName)
}
