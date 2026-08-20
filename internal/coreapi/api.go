package coreapi

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/url"
	"strings"
	"sync"
	"time"

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
	Selection               model.Selection `json:"selection,omitempty"`
	EntryID                 int64           `json:"entryID,omitempty"`
	Read                    bool            `json:"read,omitempty"`
	CurrentStarred          bool            `json:"currentStarred,omitempty"`
	DesiredStarred          bool            `json:"desiredStarred,omitempty"`
	FeedID                  int64           `json:"feedID,omitempty"`
	FeedName                string          `json:"feedName,omitempty"`
}

type Response struct {
	OK       bool                  `json:"ok"`
	Error    string                `json:"error,omitempty"`
	Text     string                `json:"text,omitempty"`
	Snapshot *model.BrowseSnapshot `json:"snapshot,omitempty"`
	Icon     *Icon                 `json:"icon,omitempty"`
}

type Icon struct {
	Regular []byte `json:"regular,omitempty"`
	Dark    []byte `json:"dark,omitempty"`
}

type Runtime struct {
	mu                      sync.Mutex
	service                 *fluxminiflux.Service
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
		service := fluxminiflux.NewWithSortOrder(server, strings.TrimSpace(request.APIKey), sortOrder, runtime.logger)
		runtime.mu.Lock()
		if request.ConfigurationGeneration >= runtime.configurationGeneration {
			runtime.service = service
			runtime.configurationGeneration = request.ConfigurationGeneration
		}
		runtime.mu.Unlock()
		return Response{OK: true}
	case "refresh":
		return runtime.snapshot(runtime.currentService(), request.Selection)
	case "set_read":
		service := runtime.currentService()
		if service == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := service.SetRead(ctx, request.EntryID, request.Read); err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return runtime.snapshot(service, request.Selection)
	case "set_starred":
		service := runtime.currentService()
		if service == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := service.SetStarred(ctx, request.EntryID, request.CurrentStarred, request.DesiredStarred); err != nil {
			return Response{OK: false, Error: err.Error()}
		}
		return runtime.snapshot(service, request.Selection)
	case "feed_icon":
		service := runtime.currentService()
		if service == nil {
			return notConfigured()
		}
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		regular, dark := service.FeedIcon(ctx, request.FeedID, request.FeedName)
		return Response{OK: true, Icon: &Icon{Regular: regular, Dark: dark}}
	default:
		return Response{OK: false, Error: fmt.Sprintf("unsupported operation %q", request.Operation)}
	}
}

func (runtime *Runtime) currentService() *fluxminiflux.Service {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	return runtime.service
}

func (runtime *Runtime) snapshot(service *fluxminiflux.Service, selection model.Selection) Response {
	if service == nil {
		return notConfigured()
	}
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	snapshot, err := service.Browse(ctx, selection)
	if err != nil {
		return Response{OK: false, Error: err.Error()}
	}
	return Response{OK: true, Snapshot: &snapshot}
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
