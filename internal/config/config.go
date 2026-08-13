package config

import (
	"errors"
	"os"
	"strings"
)

const (
	serverEnvironmentVariable = "MINIFLUX_SERVER"
	apiKeyEnvironmentVariable = "MINIFLUX_APIKEY"
)

// Config contains the credentials needed to access Miniflux.
type Config struct {
	Server string
	APIKey string
}

// Resolve prefers runtime environment variables and falls back to build-time values.
func Resolve(linkedServer, linkedAPIKey string) (Config, error) {
	config := Config{
		Server: strings.TrimSpace(linkedServer),
		APIKey: strings.TrimSpace(linkedAPIKey),
	}
	if value := strings.TrimSpace(os.Getenv(serverEnvironmentVariable)); value != "" {
		config.Server = value
	}
	if value := strings.TrimSpace(os.Getenv(apiKeyEnvironmentVariable)); value != "" {
		config.APIKey = value
	}

	if config.Server == "" || config.APIKey == "" {
		return Config{}, errors.New("MINIFLUX_SERVER und MINIFLUX_APIKEY müssen gesetzt oder beim Build eingebettet werden")
	}
	return config, nil
}
