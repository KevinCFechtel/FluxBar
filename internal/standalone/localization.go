package standalone

import (
	"embed"
	"fmt"
	"log"

	"fyne.io/fyne/v2/lang"
)

// translationFiles keeps all application translations in the Go binary so
// they can be reused by every platform-specific interface.
//
//go:embed translations
var translationFiles embed.FS

var translationLoadError = loadTranslations()

func loadTranslations() error {
	err := lang.AddTranslationsFS(translationFiles, "translations")
	if err != nil {
		log.Printf("FluxBar translations could not be loaded: %v", err)
	}
	return err
}

func localized(key, fallback string) string {
	return lang.X(key, fallback)
}

func localizedFormat(key, fallback string, arguments ...any) string {
	return fmt.Sprintf(localized(key, fallback), arguments...)
}

func localizedPlural(key, oneFallback, otherFallback string, count int, data any) string {
	fallback := otherFallback
	if count == 1 {
		fallback = oneFallback
	}
	return lang.XN(key, fallback, count, data)
}
