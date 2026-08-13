package main

import (
	"log"
	"os"
	"path/filepath"

	"github.com/KevinCFechtel/FluxBar/internal/assets"
	"github.com/KevinCFechtel/FluxBar/internal/config"
	"github.com/KevinCFechtel/FluxBar/internal/icons"
	fluxminiflux "github.com/KevinCFechtel/FluxBar/internal/miniflux"
	"github.com/KevinCFechtel/FluxBar/internal/standalone"
)

var MINIFLUX_SERVER string
var MINIFLUX_APIKEY string

func main() {
	logger := applicationLogger()
	resolved, err := config.Resolve(MINIFLUX_SERVER, MINIFLUX_APIKEY)
	if err != nil {
		logger.Fatal(err)
	}
	menuIcon, err := icons.Normalize(assets.MinifluxSVG(), "image/svg+xml", 44)
	if err != nil {
		logger.Fatalf("Menüleisten-Icon konnte nicht verarbeitet werden: %v", err)
	}
	service := fluxminiflux.New(resolved.Server, resolved.APIKey, logger)
	standalone.New(service, logger, menuIcon).Run()
}

func applicationLogger() *log.Logger {
	configDirectory, err := os.UserConfigDir()
	if err != nil {
		return log.New(os.Stderr, "", log.LstdFlags)
	}
	logDirectory := filepath.Join(configDirectory, "FluxBar")
	if err := os.MkdirAll(logDirectory, 0o700); err != nil {
		return log.New(os.Stderr, "", log.LstdFlags)
	}
	file, err := os.OpenFile(filepath.Join(logDirectory, "fluxbar.log"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return log.New(os.Stderr, "", log.LstdFlags)
	}
	return log.New(file, "", log.LstdFlags)
}
