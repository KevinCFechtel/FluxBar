package localization

import (
	"embed"
	"encoding/json"
	"fmt"
	"io/fs"

	"github.com/nicksnyder/go-i18n/v2/i18n"
	"golang.org/x/text/language"
)

//go:embed translations/*.json
var translationFiles embed.FS

var sharedBundle, translationLoadError = loadBundle()

type Localizer struct {
	localizer *i18n.Localizer
}

// New creates a localizer using ordered BCP-47 locale preferences. English is
// the catalog and message fallback when none of the preferences match.
func New(preferredLocales ...string) (*Localizer, error) {
	if translationLoadError != nil {
		return nil, translationLoadError
	}
	if len(preferredLocales) == 0 {
		preferredLocales = []string{"en"}
	}

	locales := append([]string(nil), preferredLocales...)
	locales = append(locales, "en")
	return &Localizer{localizer: i18n.NewLocalizer(sharedBundle, locales...)}, nil
}

func (l *Localizer) Text(key, fallback string) string {
	if l == nil || l.localizer == nil {
		return fallback
	}

	localized, _ := l.localizer.Localize(&i18n.LocalizeConfig{
		DefaultMessage: &i18n.Message{ID: key, Other: fallback},
	})
	if localized == "" {
		return fallback
	}
	return localized
}

func (l *Localizer) Format(key, fallback string, arguments ...any) string {
	return fmt.Sprintf(l.Text(key, fallback), arguments...)
}

func (l *Localizer) Plural(key, oneFallback, otherFallback string, count int, data any) string {
	if l == nil || l.localizer == nil {
		return renderFallback(key, oneFallback, otherFallback, count, data)
	}

	localized, _ := l.localizer.Localize(&i18n.LocalizeConfig{
		DefaultMessage: &i18n.Message{
			ID:    key,
			One:   oneFallback,
			Other: otherFallback,
		},
		PluralCount:  count,
		TemplateData: data,
	})
	if localized == "" {
		return renderFallback(key, oneFallback, otherFallback, count, data)
	}
	return localized
}

func loadBundle() (*i18n.Bundle, error) {
	bundle := i18n.NewBundle(language.English)
	bundle.RegisterUnmarshalFunc("json", json.Unmarshal)

	files, err := fs.Glob(translationFiles, "translations/*.json")
	if err != nil {
		return nil, fmt.Errorf("find translation files: %w", err)
	}
	if len(files) == 0 {
		return nil, fmt.Errorf("no translation files found")
	}
	for _, file := range files {
		if _, err := bundle.LoadMessageFileFS(translationFiles, file); err != nil {
			return nil, fmt.Errorf("load %s: %w", file, err)
		}
	}

	return bundle, nil
}

func renderFallback(key, oneFallback, otherFallback string, count int, data any) string {
	bundle := i18n.NewBundle(language.English)
	localizer := i18n.NewLocalizer(bundle, "en")
	localized, err := localizer.Localize(&i18n.LocalizeConfig{
		DefaultMessage: &i18n.Message{
			ID:    key,
			One:   oneFallback,
			Other: otherFallback,
		},
		PluralCount:  count,
		TemplateData: data,
	})
	if err == nil && localized != "" {
		return localized
	}
	if count == 1 {
		return oneFallback
	}
	return otherFallback
}
