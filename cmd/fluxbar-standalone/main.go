package main

import (
	"log"
	"os"
	"path/filepath"

	"github.com/KevinCFechtel/FluxBar/internal/assets"
	"github.com/KevinCFechtel/FluxBar/internal/icons"
	fluxminiflux "github.com/KevinCFechtel/FluxBar/internal/miniflux"
	"github.com/KevinCFechtel/FluxBar/internal/standalone"
)

func main() {
	logger := applicationLogger()
	menuIcon, err := icons.Normalize(assets.FluxBarTemplateSVG(), "image/svg+xml", 44)
	if err != nil {
		logger.Fatalf("Menüleisten-Icon konnte nicht verarbeitet werden: %v", err)
	}
	standalone.NewConfigured(
		func(settings standalone.Settings) standalone.Reader {
			sortOrder := fluxminiflux.SortOldestFirst
			if settings.NewestFirst {
				sortOrder = fluxminiflux.SortNewestFirst
			}
			return fluxminiflux.NewWithSortOrder(settings.Server, settings.APIKey, sortOrder, logger)
		},
		standalone.NewNativeSettings(),
		logger,
		menuIcon,
	).Run()
}

func applicationLogger() *log.Logger {
	configDirectory, err := os.UserConfigDir()
	if err != nil {
		return log.New(os.Stderr, "", log.LstdFlags|log.Lmicroseconds)
	}
	logDirectory := filepath.Join(configDirectory, "FluxBar")
	if err := os.MkdirAll(logDirectory, 0o700); err != nil {
		return log.New(os.Stderr, "", log.LstdFlags|log.Lmicroseconds)
	}
	file, err := os.OpenFile(filepath.Join(logDirectory, "fluxbar.log"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return log.New(os.Stderr, "", log.LstdFlags|log.Lmicroseconds)
	}
	return log.New(file, "", log.LstdFlags|log.Lmicroseconds)
}
