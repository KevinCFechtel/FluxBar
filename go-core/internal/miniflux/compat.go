//go:build compat
// +build compat

package miniflux

import (
	"github.com/KevinCFechtel/FluxBar/internal/model"
	miniflux "miniflux.app/v2/client"
)

// MapEntryForCompat exposes the production entry mapping for cross-language
// compatibility probes. It is not used by the application runtime.
func MapEntryForCompat(source *miniflux.Entry) model.Entry {
	entries := mapEntries(miniflux.Entries{source})
	if len(entries) == 0 {
		return model.Entry{}
	}
	return entries[0]
}
