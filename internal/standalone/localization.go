package standalone

import (
	"log"

	"github.com/KevinCFechtel/FluxBar/internal/localization"
	locale "github.com/jeandeaual/go-locale"
)

var appLocalizer, translationLoadError = loadTranslations()

func loadTranslations() (*localization.Localizer, error) {
	locales, err := locale.GetLocales()
	if err != nil {
		log.Printf("FluxBar user locales could not be loaded: %v", err)
		locales = []string{"en"}
	}

	localizer, err := localization.New(locales...)
	if err != nil {
		log.Printf("FluxBar translations could not be loaded: %v", err)
	}
	return localizer, err
}

func localized(key, fallback string) string {
	return appLocalizer.Text(key, fallback)
}

func localizedFormat(key, fallback string, arguments ...any) string {
	return appLocalizer.Format(key, fallback, arguments...)
}

func localizedPlural(key, oneFallback, otherFallback string, count int, data any) string {
	return appLocalizer.Plural(key, oneFallback, otherFallback, count, data)
}
