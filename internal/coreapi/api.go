package coreapi

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/inbox"
	"github.com/KevinCFechtel/FluxBar/internal/localization"
	fluxminiflux "github.com/KevinCFechtel/FluxBar/internal/miniflux"
	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type Request struct {
	Operation               string          `json:"operation"`
	Server                  string          `json:"server,omitempty"`
	APIKey                  string          `json:"apiKey,omitempty"`
	NewestFirst             bool            `json:"newestFirst,omitempty"`
	ConfigurationGeneration int64           `json:"configurationGeneration,omitempty"`
	Locales                 []string        `json:"locales,omitempty"`
	Key                     string          `json:"key,omitempty"`
	Fallback                string          `json:"fallback,omitempty"`
	OneFallback             string          `json:"oneFallback,omitempty"`
	OtherFallback           string          `json:"otherFallback,omitempty"`
	Count                   int             `json:"count,omitempty"`
	Selection               model.Selection `json:"selection,omitempty"`
	EntryID                 int64           `json:"entryID,omitempty"`
	EntryIDs                []int64         `json:"entryIDs,omitempty"`
	RetainEntryIDs          []int64         `json:"retainEntryIDs,omitempty"`
	Read                    bool            `json:"read,omitempty"`
	MutationSource          string          `json:"mutationSource,omitempty"`
	MutationID              string          `json:"mutationID,omitempty"`
	CurrentStarred          bool            `json:"currentStarred,omitempty"`
	DesiredStarred          bool            `json:"desiredStarred,omitempty"`
	FeedID                  int64           `json:"feedID,omitempty"`
	FeedName                string          `json:"feedName,omitempty"`
}

type Response struct {
	OK       bool                   `json:"ok"`
	Error    string                 `json:"error,omitempty"`
	Text     string                 `json:"text,omitempty"`
	Snapshot *model.BrowseSnapshot  `json:"snapshot,omitempty"`
	Icon     *Icon                  `json:"icon,omitempty"`
	Receipt  *inbox.MutationReceipt `json:"receipt,omitempty"`
}

type Icon struct {
	Regular []byte `json:"regular,omitempty"`
	Dark    []byte `json:"dark,omitempty"`
}

type Runtime struct {
	mu                      sync.Mutex
	engine                  *inbox.Service
	store                   *inbox.Store
	logger                  *log.Logger
	configurationGeneration int64
}

func New(logger *log.Logger) *Runtime {
	if logger == nil {
		logger = log.Default()
	}
	return &Runtime{logger: logger}
}

func (runtime *Runtime) HandleJSON(input string) string {
	var request Request
	if err := json.Unmarshal([]byte(input), &request); err != nil {
		return encode(Response{OK: false, Error: "invalid request: " + err.Error()})
	}
	response := runtime.handle(request)
	return encode(response)
}

func (runtime *Runtime) handle(request Request) Response {
	if request.Operation == "localize" {
		localizer, err := localization.New(request.Locales...)
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true, Text: localizer.Text(request.Key, request.Fallback)}
	}
	if request.Operation == "localize_plural" {
		localizer, err := localization.New(request.Locales...)
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true, Text: localizer.Plural(request.Key, request.OneFallback, request.OtherFallback, request.Count, map[string]any{"Count": request.Count})}
	}

	switch request.Operation {
	case "configure":
		server, err := validateConfiguration(request.Server, request.APIKey, request.Locales)
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		sortOrder := fluxminiflux.SortOldestFirst
		if request.NewestFirst {
			sortOrder = fluxminiflux.SortNewestFirst
		}
		store, err := runtime.currentStore()
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		apiKey := strings.TrimSpace(request.APIKey)
		accountID := inbox.AccountID(server, apiKey)
		if err := store.EnsureAccount(context.Background(), accountID, server); err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		remote := fluxminiflux.NewWithSortOrder(server, apiKey, sortOrder, runtime.logger)
		engine := inbox.NewService(store, remote, accountID, request.NewestFirst, runtime.logger)
		runtime.mu.Lock()
		if request.ConfigurationGeneration >= runtime.configurationGeneration {
			runtime.engine = engine
			runtime.configurationGeneration = request.ConfigurationGeneration
		}
		runtime.mu.Unlock()
		return Response{OK: true}
	case "local_snapshot":
		return runtime.localSnapshot(runtime.currentEngine(), request.Selection, request.RetainEntryIDs...)
	case "refresh":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
		defer cancel()
		snapshot, err := engine.Sync(ctx, request.Selection, request.RetainEntryIDs...)
		if err != nil {
			if snapshot.Version > 0 {
				return Response{OK: true, Error: err.Error(), Snapshot: &snapshot}
			}
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true, Snapshot: &snapshot}
	case "set_read":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ids := append([]int64(nil), request.EntryIDs...)
		if len(ids) == 0 && request.EntryID > 0 {
			ids = []int64{request.EntryID}
		}
		if len(ids) == 0 {
			return Response{OK: false, Error: "missing entry IDs"}
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		snapshot, receipt, err := engine.MarkRead(ctx, request.Selection, ids, request.RetainEntryIDs, request.Read, request.MutationSource == "automatic")
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true, Snapshot: &snapshot, Receipt: receipt}
	case "set_starred":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		snapshot, err := engine.SetStarred(ctx, request.Selection, request.EntryID, request.DesiredStarred, request.RetainEntryIDs)
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true, Snapshot: &snapshot}
	case "undo_read":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		snapshot, err := engine.Undo(ctx, request.Selection, request.MutationID, request.RetainEntryIDs)
		if err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true, Snapshot: &snapshot}
	case "discard_undo":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := engine.DiscardUndo(ctx, request.MutationID); err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return Response{OK: true}
	case "flush_pending":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		if err := engine.Flush(ctx); err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return runtime.localSnapshot(engine, request.Selection, request.RetainEntryIDs...)
	case "feed_icon":
		engine := runtime.currentEngine()
		if engine == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		regular, dark := engine.FeedIcon(ctx, request.FeedID, request.FeedName)
		return Response{OK: true, Icon: &Icon{Regular: regular, Dark: dark}}
	default:
		return Response{OK: false, Error: fmt.Sprintf("unsupported operation %q", request.Operation)}
	}
}

func (runtime *Runtime) currentEngine() *inbox.Service {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	return runtime.engine
}

func (runtime *Runtime) localSnapshot(engine *inbox.Service, selection model.Selection, retainIDs ...int64) Response {
	if engine == nil {
		return notConfigured()
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	snapshot, err := engine.LocalSnapshot(ctx, selection, retainIDs...)
	if err != nil {
		return Response{OK: false, Error: err.Error()}
	}
	return Response{OK: true, Snapshot: &snapshot}
}

func (runtime *Runtime) currentStore() (*inbox.Store, error) {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	if runtime.store != nil {
		return runtime.store, nil
	}
	directory, err := os.UserConfigDir()
	if err != nil {
		return nil, fmt.Errorf("Anwendungsdaten-Verzeichnis bestimmen: %w", err)
	}
	directory = filepath.Join(directory, "FluxBar")
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return nil, fmt.Errorf("Anwendungsdaten-Verzeichnis anlegen: %w", err)
	}
	store, err := inbox.OpenStore(filepath.Join(directory, "inbox.sqlite3"))
	if err != nil {
		return nil, err
	}
	runtime.store = store
	return store, nil
}

func notConfigured() Response {
	return Response{OK: false, Error: "Miniflux is not configured"}
}

func validateConfiguration(server, apiKey string, locales []string) (string, error) {
	localizer, _ := localization.New(locales...)
	server = strings.TrimRight(strings.TrimSpace(server), "/")
	parsed, err := url.Parse(server)
	if err != nil || parsed.Host == "" || parsed.Scheme != "http" && parsed.Scheme != "https" {
		return "", fmt.Errorf("%s", localizer.Text("validation.server_invalid", "The server URL must be a complete HTTP or HTTPS URL."))
	}
	if strings.TrimSpace(apiKey) == "" {
		return "", fmt.Errorf("%s", localizer.Text("validation.api_key_required", "Please enter a Miniflux API key."))
	}
	return server, nil
}

func encode(response Response) string {
	data, err := json.Marshal(response)
	if err != nil {
		return `{"ok":false,"error":"encode response"}`
	}
	return string(data)
}
