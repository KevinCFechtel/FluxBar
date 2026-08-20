package standalone

import (
	"context"
	"errors"
	"log"
	"net/url"
	"os/exec"
	"strconv"
	"strings"
	"sync/atomic"
	"time"
	"unicode/utf8"

	"fyne.io/systray"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type Reader interface {
	Unread(context.Context) ([]model.Entry, int, error)
	MarkRead(context.Context, ...int64) error
}

type ReaderFactory func(Settings) Reader

var openBrowser = func(target string) error {
	return exec.Command("open", target).Start()
}

type App struct {
	reader           Reader
	readerFactory    ReaderFactory
	settingsEditor   SettingsEditor
	logger           *log.Logger
	icon             []byte
	refresh          chan struct{}
	refreshResults   chan refreshResult
	appearanceSignal chan struct{}
	settingsRequest  chan struct{}
	settingsResults  chan settingsResult
	appearanceDark   atomic.Bool
	stopAppearance   func()

	// The event loop owns the fields below. Keeping menu state in the long-lived
	// process lets appearance changes redraw the menu without another API call.
	entries         []model.Entry
	total           int
	hasData         bool
	darkMode        bool
	refreshRunning  bool
	refreshPending  bool
	settingsEditing bool
	settings        Settings
	configurationID uint64
}

type refreshResult struct {
	entries         []model.Entry
	total           int
	err             error
	configurationID uint64
}

type settingsResult struct {
	settings Settings
	saved    bool
	err      error
}

func New(reader Reader, logger *log.Logger, icon []byte) *App {
	return &App{
		reader:           reader,
		logger:           logger,
		icon:             icon,
		refresh:          make(chan struct{}, 1),
		refreshResults:   make(chan refreshResult),
		appearanceSignal: make(chan struct{}, 1),
		settingsRequest:  make(chan struct{}, 1),
		settingsResults:  make(chan settingsResult),
	}
}

func NewConfigured(factory ReaderFactory, editor SettingsEditor, logger *log.Logger, icon []byte) *App {
	app := New(nil, logger, icon)
	app.readerFactory = factory
	app.settingsEditor = editor
	app.settings.ShowSplash = true
	return app
}

func (app *App) Run() {
	systray.Run(app.ready, app.exit)
}

func (app *App) ready() {
	if initializeArticleHover() {
		app.logger.Printf("level=info component=preview event=hover_initialized delay_ms=500")
	} else {
		app.logger.Printf("level=error component=preview event=hover_initialization_failed")
	}
	if len(app.icon) > 0 {
		systray.SetTemplateIcon(app.icon, app.icon)
	}
	systray.SetTooltip(localized("app.tooltip.default", "FluxNews — Miniflux"))
	app.renderMessage(localized("status.loading", "Loading Miniflux…"), true)

	app.darkMode = darkAppearance()
	app.appearanceDark.Store(app.darkMode)
	var observingAppearance bool
	app.stopAppearance, observingAppearance = observeAppearance(app.notifyAppearance)
	if !observingAppearance {
		app.logger.Printf("level=error component=appearance event=observation_failed")
	}
	if app.settingsEditor == nil {
		go app.eventLoop()
		go app.scheduleRefresh()
		app.requestRefresh()
		return
	}
	settings, err := app.settingsEditor.Load()
	if err == nil {
		app.applySettings(settings)
	} else if errors.Is(err, ErrSettingsNotFound) {
		app.renderMessage(localized("status.configuration_required", "Configuration required"), true)
	} else {
		app.logger.Printf("Einstellungen konnten nicht geladen werden: %v", err)
		app.renderError(err.Error())
	}
	if app.settings.ShowSplash {
		showStartupSplash()
	}
	go app.eventLoop()
	go app.scheduleRefresh()
	if app.reader != nil {
		app.requestRefresh()
	}
}

func (app *App) exit() {
	if app.stopAppearance != nil {
		app.stopAppearance()
	}
	closeArticleHover()
}

func (app *App) scheduleRefresh() {
	ticker := time.NewTicker(15 * time.Minute)
	defer ticker.Stop()
	for range ticker.C {
		app.requestRefresh()
	}
}

func (app *App) requestRefresh() {
	select {
	case app.refresh <- struct{}{}:
	default:
	}
}

func (app *App) requestSettings() {
	select {
	case app.settingsRequest <- struct{}{}:
	default:
	}
}

func (app *App) notifyAppearance(dark bool) {
	app.appearanceDark.Store(dark)
	select {
	case app.appearanceSignal <- struct{}{}:
	default:
		// A pending signal will read the most recently stored value.
	}
}

func (app *App) eventLoop() {
	for {
		select {
		case <-app.refresh:
			if app.refreshRunning {
				app.refreshPending = true
				continue
			}
			app.startRefresh()
		case result := <-app.refreshResults:
			app.refreshRunning = false
			if result.configurationID != app.configurationID {
				if app.refreshPending {
					app.refreshPending = false
					app.startRefresh()
				}
				continue
			}
			if result.err != nil {
				app.entries = nil
				app.total = 0
				app.hasData = false
				app.logger.Printf("Aktualisierung fehlgeschlagen: %v", result.err)
				app.renderError(localized("status.refresh_failed", "Refresh failed"))
			} else {
				app.entries = result.entries
				app.total = result.total
				app.hasData = true
				app.render(app.entries, app.total, app.darkMode)
			}
			if app.refreshPending {
				app.refreshPending = false
				app.startRefresh()
			}
		case <-app.appearanceSignal:
			dark := app.appearanceDark.Load()
			if dark == app.darkMode {
				continue
			}
			app.darkMode = dark
			app.logger.Printf("level=info component=appearance event=changed dark=%t", dark)
			if app.hasData {
				app.render(app.entries, app.total, dark)
			}
		case <-app.settingsRequest:
			if app.settingsEditor == nil || app.settingsEditing {
				continue
			}
			app.settingsEditing = true
			current := app.settings
			go func() {
				settings, saved, err := app.settingsEditor.Edit(current)
				app.settingsResults <- settingsResult{settings: settings, saved: saved, err: err}
			}()
		case result := <-app.settingsResults:
			app.settingsEditing = false
			if result.err != nil {
				app.logger.Printf("Einstellungen konnten nicht gespeichert werden: %v", result.err)
				app.renderError(result.err.Error())
				continue
			}
			if !result.saved {
				continue
			}
			app.applySettings(result.settings)
			app.entries = nil
			app.total = 0
			app.hasData = false
			app.renderMessage(localized("status.loading", "Loading Miniflux…"), true)
			if app.refreshRunning {
				app.refreshPending = true
			} else {
				app.requestRefresh()
			}
		}
	}
}

func (app *App) applySettings(settings Settings) {
	app.settings = settings
	app.reader = app.readerFactory(settings)
	app.configurationID++
}

func (app *App) startRefresh() {
	if app.reader == nil {
		return
	}
	app.refreshRunning = true
	reader := app.reader
	configurationID := app.configurationID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
		entries, total, err := reader.Unread(ctx)
		cancel()
		app.refreshResults <- refreshResult{
			entries: entries, total: total, err: err, configurationID: configurationID,
		}
	}()
}

func (app *App) render(entries []model.Entry, total int, darkMode bool) {
	resetArticleHover()
	systray.ResetMenu()
	systray.SetTitle(strconv.Itoa(total))
	systray.SetTooltip(unreadTooltip(total))
	if len(entries) == 0 {
		item := systray.AddMenuItem(localized("status.no_unread", "No unread articles"), "")
		item.Disable()
	} else {
		reader := app.reader
		for _, entry := range entries {
			entry := entry
			item := systray.AddMenuItem(menuLabel(entry), "")
			registerArticleHover(entry)
			if icon := iconForAppearance(entry, darkMode); len(icon) > 0 {
				item.SetIcon(icon)
			}
			go func() {
				for range item.ClickedCh {
					app.openAndMarkRead(reader, entry)
				}
			}()
		}
	}
	app.addFooter()
}

func iconForAppearance(entry model.Entry, darkMode bool) []byte {
	if darkMode && len(entry.DarkIcon) > 0 {
		return entry.DarkIcon
	}
	return entry.Icon
}

func (app *App) renderError(message string) {
	systray.SetTitle("!")
	systray.SetTooltip(localized("app.tooltip.error", "FluxNews — Error"))
	app.renderMessage(localizedFormat("status.error_format", "Error: %s", truncate(message, 100)), true)
}

func (app *App) renderMessage(message string, footer bool) {
	resetArticleHover()
	systray.ResetMenu()
	item := systray.AddMenuItem(message, "")
	item.Disable()
	if footer {
		app.addFooter()
	}
}

func (app *App) addFooter() {
	systray.AddSeparator()
	refresh := systray.AddMenuItem(
		localized("menu.refresh", "Refresh"),
		localized("menu.refresh.tooltip", "Refresh Miniflux now"),
	)
	if app.reader == nil {
		refresh.Disable()
	}
	var settings *systray.MenuItem
	if app.settingsEditor != nil {
		settings = systray.AddMenuItem(
			localized("menu.settings", "Settings…"),
			localized("menu.settings.tooltip", "Edit Miniflux credentials"),
		)
	}
	quit := systray.AddMenuItem(localized("menu.quit", "Quit FluxNews"), "")
	go func() {
		for range refresh.ClickedCh {
			app.requestRefresh()
		}
	}()
	if settings != nil {
		go func() {
			for range settings.ClickedCh {
				app.requestSettings()
			}
		}()
	}
	go func() {
		for range quit.ClickedCh {
			systray.Quit()
		}
	}()
}

func unreadTooltip(total int) string {
	return localizedPlural(
		"status.unread_count",
		"FluxNews — {{.Count}} unread article",
		"FluxNews — {{.Count}} unread articles",
		total,
		map[string]any{"Count": total},
	)
}

func (app *App) openAndMarkRead(reader Reader, entry model.Entry) {
	parsed, err := url.Parse(entry.URL)
	if err != nil || (parsed.Scheme != "http" && parsed.Scheme != "https") {
		app.logger.Printf("Ungültige Artikel-URL %q", entry.URL)
		return
	}
	if err := openBrowser(entry.URL); err != nil {
		app.logger.Printf("Artikel konnte nicht geöffnet werden: %v", err)
		return
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	if err := reader.MarkRead(ctx, entry.ID); err != nil {
		app.logger.Print(err)
	}
	cancel()
	app.requestRefresh()
}

func menuLabel(entry model.Entry) string {
	feed := strings.TrimSpace(strings.ReplaceAll(entry.FeedName, "\n", " "))
	title := strings.TrimSpace(strings.ReplaceAll(entry.Title, "\n", " "))
	if feed == "" {
		return truncate(title, 120)
	}
	return truncate(feed+": "+title, 120)
}

func truncate(value string, limit int) string {
	if utf8.RuneCountInString(value) <= limit {
		return value
	}
	runes := []rune(value)
	return string(runes[:limit-1]) + "…"
}
