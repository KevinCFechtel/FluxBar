package standalone

import (
	"context"
	"fmt"
	"log"
	"net/url"
	"os/exec"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"fyne.io/systray"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type Reader interface {
	Unread(context.Context) ([]model.Entry, int, error)
	MarkRead(context.Context, ...int64) error
}

type App struct {
	reader  Reader
	logger  *log.Logger
	icon    []byte
	refresh chan struct{}
}

func New(reader Reader, logger *log.Logger, icon []byte) *App {
	return &App{
		reader:  reader,
		logger:  logger,
		icon:    icon,
		refresh: make(chan struct{}, 1),
	}
}

func (app *App) Run() {
	systray.Run(app.ready, func() {})
}

func (app *App) ready() {
	if len(app.icon) > 0 {
		systray.SetTemplateIcon(app.icon, app.icon)
	}
	systray.SetTooltip("FluxBar – Miniflux")
	app.renderMessage("Miniflux wird geladen …", true)

	go app.refreshLoop()
	go app.scheduleRefresh()
	app.requestRefresh()
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

func (app *App) refreshLoop() {
	for range app.refresh {
		ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
		entries, total, err := app.reader.Unread(ctx)
		cancel()
		if err != nil {
			app.logger.Printf("Aktualisierung fehlgeschlagen: %v", err)
			app.renderError(err)
			continue
		}
		app.render(entries, total)
	}
}

func (app *App) render(entries []model.Entry, total int) {
	systray.ResetMenu()
	systray.SetTitle(strconv.Itoa(total))
	systray.SetTooltip(fmt.Sprintf("FluxBar – %d ungelesene Artikel", total))
	if len(entries) == 0 {
		item := systray.AddMenuItem("Keine ungelesenen Artikel", "")
		item.Disable()
	} else {
		for _, entry := range entries {
			entry := entry
			item := systray.AddMenuItem(menuLabel(entry), entry.URL)
			if len(entry.Icon) > 0 {
				item.SetIcon(entry.Icon)
			}
			go func() {
				for range item.ClickedCh {
					app.openAndMarkRead(entry)
				}
			}()
		}
	}
	app.addFooter()
}

func (app *App) renderError(err error) {
	systray.SetTitle("!")
	systray.SetTooltip("FluxBar – Aktualisierung fehlgeschlagen")
	app.renderMessage("Fehler: "+truncate(err.Error(), 100), true)
}

func (app *App) renderMessage(message string, footer bool) {
	systray.ResetMenu()
	item := systray.AddMenuItem(message, "")
	item.Disable()
	if footer {
		app.addFooter()
	}
}

func (app *App) addFooter() {
	systray.AddSeparator()
	refresh := systray.AddMenuItem("Aktualisieren", "Miniflux jetzt aktualisieren")
	quit := systray.AddMenuItem("FluxBar beenden", "")
	go func() {
		for range refresh.ClickedCh {
			app.requestRefresh()
		}
	}()
	go func() {
		for range quit.ClickedCh {
			systray.Quit()
		}
	}()
}

func (app *App) openAndMarkRead(entry model.Entry) {
	parsed, err := url.Parse(entry.URL)
	if err != nil || (parsed.Scheme != "http" && parsed.Scheme != "https") {
		app.logger.Printf("Ungültige Artikel-URL %q", entry.URL)
		return
	}
	if err := exec.Command("open", entry.URL).Start(); err != nil {
		app.logger.Printf("Artikel konnte nicht geöffnet werden: %v", err)
		return
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	if err := app.reader.MarkRead(ctx, entry.ID); err != nil {
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
