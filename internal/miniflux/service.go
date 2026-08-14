package miniflux

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/article"
	"github.com/KevinCFechtel/FluxBar/internal/icons"
	"github.com/KevinCFechtel/FluxBar/internal/model"
	miniflux "miniflux.app/v2/client"
)

const iconWorkers = 6

type SortOrder string

const (
	SortOldestFirst SortOrder = "oldest_first"
	SortNewestFirst SortOrder = "newest_first"
)

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
	sort   SortOrder

	iconMu    sync.RWMutex
	iconCache map[int64]cachedIcon

	debugIcons       bool
	dumpFailedIcons  bool
	iconDumpDir      string
	backgroundAlways map[int64]bool
	backgroundNever  map[int64]bool
}

type cachedIcon struct {
	regular []byte
	dark    []byte
}

func New(server, apiKey string, logger *log.Logger) *Service {
	return NewWithSortOrder(server, apiKey, SortOldestFirst, logger)
}

func NewWithSortOrder(server, apiKey string, sort SortOrder, logger *log.Logger) *Service {
	return newWithClient(miniflux.NewClient(server, apiKey), sort, logger)
}

func NewWithClient(client client, logger logger) *Service {
	return newWithClient(client, SortOldestFirst, logger)
}

func newWithClient(client client, sort SortOrder, logger logger) *Service {
	if logger == nil {
		logger = log.Default()
	}
	if sort != SortNewestFirst {
		sort = SortOldestFirst
	}
	service := &Service{
		client:           client,
		logger:           logger,
		sort:             sort,
		iconCache:        make(map[int64]cachedIcon),
		debugIcons:       environmentEnabled("FLUXBAR_DEBUG_ICONS"),
		dumpFailedIcons:  environmentEnabled("FLUXBAR_DUMP_FAILED_ICONS"),
		iconDumpDir:      iconDumpDirectory(),
		backgroundAlways: feedIDSet(os.Getenv("FLUXBAR_ICON_BACKGROUND_ALWAYS")),
		backgroundNever:  feedIDSet(os.Getenv("FLUXBAR_ICON_BACKGROUND_NEVER")),
	}
	if service.debugIcons || service.dumpFailedIcons {
		service.logger.Printf(
			"level=info component=icons event=diagnostics_enabled debug=%t dump_failed=%t dump_dir=%q",
			service.debugIcons, service.dumpFailedIcons, service.iconDumpDir,
		)
	}
	return service
}

// Unread returns all unread entries in the configured publication order.
func (service *Service) Unread(ctx context.Context) ([]model.Entry, int, error) {
	direction := "asc"
	if service.sort == SortNewestFirst {
		direction = "desc"
	}
	result, err := service.client.EntriesContext(ctx, &miniflux.Filter{
		Status:    miniflux.EntryStatusUnread,
		Order:     "published_at",
		Direction: direction,
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
		preview := article.Extract(source.Content, source.URL, article.PreviewLimit)
		entries = append(entries, model.Entry{
			ID:          source.ID,
			Title:       source.Title,
			URL:         source.URL,
			FeedID:      feedID,
			FeedName:    feedName,
			PublishedAt: source.Date,
			Preview:     preview.Text,
			ImageURL:    preview.ImageURL,
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
		icon   cachedIcon
		cached bool
		failed bool
	}
	type job struct {
		feedID   int64
		feedName string
	}

	started := time.Now()
	jobs := make(chan job)
	results := make(chan result)
	workerCount := min(iconWorkers, len(feedEntries))
	var workers sync.WaitGroup
	for range workerCount {
		workers.Add(1)
		go func() {
			defer workers.Done()
			for job := range jobs {
				icon, cached := service.icon(ctx, job.feedID, job.feedName)
				results <- result{feedID: job.feedID, icon: icon, cached: cached, failed: len(icon.regular) == 0}
			}
		}()
	}

	go func() {
		for feedID := range feedEntries {
			indexes := feedEntries[feedID]
			feedName := ""
			if len(indexes) > 0 {
				feedName = entries[indexes[0]].FeedName
			}
			jobs <- job{feedID: feedID, feedName: feedName}
		}
		close(jobs)
		workers.Wait()
		close(results)
	}()

	succeeded, failed, cached := 0, 0, 0
	for loaded := range results {
		if loaded.failed {
			failed++
		} else {
			succeeded++
		}
		if loaded.cached {
			cached++
		}
		for _, index := range feedEntries[loaded.feedID] {
			entries[index].Icon = loaded.icon.regular
			entries[index].DarkIcon = loaded.icon.dark
		}
	}
	if service.debugIcons {
		service.logger.Printf(
			"level=debug component=icons event=summary feeds=%d success=%d failed=%d cached=%d duration_ms=%d",
			len(feedEntries), succeeded, failed, cached, time.Since(started).Milliseconds(),
		)
	}
}

func (service *Service) icon(ctx context.Context, feedID int64, feedName string) (cachedIcon, bool) {
	service.iconMu.RLock()
	icon, known := service.iconCache[feedID]
	service.iconMu.RUnlock()
	if known {
		if service.debugIcons {
			service.logger.Printf(
				"level=debug component=icon event=cache_hit feed_id=%d feed=%q output_bytes=%d dark_output_bytes=%d",
				feedID, logValue(feedName), len(icon.regular), len(icon.dark),
			)
		}
		return icon, true
	}

	started := time.Now()
	feedIcon, err := service.client.FeedIconContext(ctx, feedID)
	if err != nil {
		service.logger.Printf(
			"level=error component=icon event=failed feed_id=%d feed=%q stage=fetch duration_ms=%d error=%q",
			feedID, logValue(feedName), time.Since(started).Milliseconds(), err.Error(),
		)
		return cachedIcon{}, false
	}
	if feedIcon == nil {
		service.logger.Printf(
			"level=error component=icon event=failed feed_id=%d feed=%q stage=fetch duration_ms=%d error=%q",
			feedID, logValue(feedName), time.Since(started).Milliseconds(), "Miniflux lieferte kein Icon",
		)
		return cachedIcon{}, false
	}
	regular, diagnostic, err := icons.NormalizeDataURLWithDiagnostic(feedIcon.Data, icons.DefaultSize)
	if err != nil {
		service.logIconDiagnostic("error", "failed", feedID, feedName, feedIcon, diagnostic, time.Since(started), err)
		if service.dumpFailedIcons {
			path, dumpErr := service.dumpFailedIcon(feedID, feedIcon.Data, diagnostic)
			if dumpErr != nil {
				service.logger.Printf("level=error component=icon event=dump_failed feed_id=%d error=%q", feedID, dumpErr.Error())
			} else if path != "" {
				service.logger.Printf("level=info component=icon event=dumped feed_id=%d path=%q", feedID, path)
			}
		}
		return cachedIcon{}, false
	}
	mode := service.backgroundMode(feedID)
	dark, analysis, appearanceErr := icons.DarkModeVariant(regular, mode)
	diagnostic.MeanLuminance = analysis.MeanLuminance
	diagnostic.DarkContrast = analysis.DarkContrast
	diagnostic.LowContrastRatio = analysis.LowContrastRatio
	diagnostic.HasTransparency = analysis.HasTransparency
	diagnostic.TransparentRatio = analysis.TransparentRatio
	diagnostic.ClassifiedDark = analysis.ClassifiedDark
	diagnostic.BackgroundMode = analysis.BackgroundMode
	diagnostic.BackgroundAdded = analysis.BackgroundAdded
	if appearanceErr != nil {
		service.logger.Printf(
			"level=error component=icon event=appearance_failed feed_id=%d feed=%q error=%q",
			feedID, logValue(feedName), appearanceErr.Error(),
		)
	}
	icon = cachedIcon{regular: regular, dark: dark}
	service.storeIcon(feedID, icon)
	if service.debugIcons {
		service.logIconDiagnostic("debug", "processed", feedID, feedName, feedIcon, diagnostic, time.Since(started), nil)
	}
	return icon, false
}

func (service *Service) logIconDiagnostic(level, event string, feedID int64, feedName string, feedIcon *miniflux.FeedIcon, diagnostic icons.Diagnostic, duration time.Duration, processingError error) {
	errorText := ""
	if processingError != nil {
		errorText = processingError.Error()
	}
	service.logger.Printf(
		"level=%s component=icon event=%s feed_id=%d feed=%q stage=%s api_icon_id=%d api_mime=%q declared_mime=%q detected_mime=%q encoded_bytes=%d decoded_bytes=%d fingerprint=%q decoded_format=%q dimensions=%q svg_viewbox=%q output_bytes=%d mean_luminance=%.3f dark_contrast=%.2f low_contrast_ratio=%.2f has_transparency=%t transparent_ratio=%.3f classified_dark=%t background_mode=%s background_added=%t duration_ms=%d error=%q",
		level,
		event,
		feedID,
		logValue(feedName),
		diagnostic.Stage,
		feedIcon.ID,
		feedIcon.MimeType,
		diagnostic.DeclaredMediaType,
		diagnostic.DetectedMediaType,
		diagnostic.EncodedBytes,
		diagnostic.DecodedBytes,
		diagnostic.Fingerprint,
		diagnostic.DecodedFormat,
		dimensions(diagnostic.Width, diagnostic.Height),
		diagnostic.SVGViewBox,
		diagnostic.OutputBytes,
		diagnostic.MeanLuminance,
		diagnostic.DarkContrast,
		diagnostic.LowContrastRatio,
		diagnostic.HasTransparency,
		diagnostic.TransparentRatio,
		diagnostic.ClassifiedDark,
		diagnostic.BackgroundMode,
		diagnostic.BackgroundAdded,
		duration.Milliseconds(),
		errorText,
	)
}

func (service *Service) dumpFailedIcon(feedID int64, dataURL string, diagnostic icons.Diagnostic) (string, error) {
	if service.iconDumpDir == "" {
		return "", fmt.Errorf("kein Verzeichnis für Icon-Dumps verfügbar")
	}
	mediaType, data, err := icons.DecodeDataURL(dataURL)
	if err != nil {
		return "", fmt.Errorf("Rohdaten können nicht dekodiert werden: %w", err)
	}
	if err := os.MkdirAll(service.iconDumpDir, 0o700); err != nil {
		return "", fmt.Errorf("Dump-Verzeichnis anlegen: %w", err)
	}
	fingerprint := diagnostic.Fingerprint
	if fingerprint == "" {
		fingerprint = "unknown"
	}
	extension := iconExtension(diagnostic.DetectedMediaType)
	if extension == "bin" {
		extension = iconExtension(mediaType)
	}
	path := filepath.Join(service.iconDumpDir, fmt.Sprintf("feed-%d-%s.%s", feedID, fingerprint, extension))
	if err := os.WriteFile(path, data, 0o600); err != nil {
		return "", fmt.Errorf("Icon-Dump schreiben: %w", err)
	}
	return path, nil
}

func (service *Service) storeIcon(feedID int64, icon cachedIcon) {
	service.iconMu.Lock()
	service.iconCache[feedID] = icon
	service.iconMu.Unlock()
}

func (service *Service) backgroundMode(feedID int64) icons.BackgroundMode {
	if service.backgroundNever[feedID] {
		return icons.BackgroundNever
	}
	if service.backgroundAlways[feedID] {
		return icons.BackgroundAlways
	}
	return icons.BackgroundAuto
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

func environmentEnabled(name string) bool {
	value := strings.TrimSpace(os.Getenv(name))
	enabled, err := strconv.ParseBool(value)
	return err == nil && enabled
}

func iconDumpDirectory() string {
	if configured := strings.TrimSpace(os.Getenv("FLUXBAR_ICON_DUMP_DIR")); configured != "" {
		return configured
	}
	configDirectory, err := os.UserConfigDir()
	if err != nil {
		return ""
	}
	return filepath.Join(configDirectory, "FluxBar", "icon-debug")
}

func iconExtension(mediaType string) string {
	switch strings.ToLower(mediaType) {
	case "image/png":
		return "png"
	case "image/jpeg":
		return "jpg"
	case "image/gif":
		return "gif"
	case "image/webp":
		return "webp"
	case "image/svg+xml":
		return "svg"
	case "image/x-icon", "image/vnd.microsoft.icon":
		return "ico"
	case "image/avif":
		return "avif"
	case "image/heif", "image/heic":
		return "heic"
	default:
		return "bin"
	}
}

func dimensions(width, height int) string {
	if width <= 0 || height <= 0 {
		return ""
	}
	return fmt.Sprintf("%dx%d", width, height)
}

func logValue(value string) string {
	value = strings.NewReplacer("\n", " ", "\r", " ", "\t", " ").Replace(value)
	value = strings.TrimSpace(value)
	if len([]rune(value)) > 120 {
		value = string([]rune(value)[:119]) + "…"
	}
	return value
}

func feedIDSet(value string) map[int64]bool {
	result := make(map[int64]bool)
	for _, part := range strings.Split(value, ",") {
		feedID, err := strconv.ParseInt(strings.TrimSpace(part), 10, 64)
		if err == nil && feedID > 0 {
			result[feedID] = true
		}
	}
	return result
}
