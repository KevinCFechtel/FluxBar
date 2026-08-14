package standalone

import (
	"errors"
	"net/url"
	"strings"
)

var ErrSettingsNotFound = errors.New("keine Miniflux-Einstellungen gespeichert")

// Settings contains the standalone Miniflux configuration. It is stored as a
// generic password in the macOS Keychain.
type Settings struct {
	Server        string
	APIKey        string
	ShowSplash    bool
	LaunchAtLogin bool
	NewestFirst   bool
}

type SettingsEditor interface {
	Load() (Settings, error)
	Edit(Settings) (Settings, bool, error)
}

type NativeSettings struct{}

func NewNativeSettings() *NativeSettings {
	return &NativeSettings{}
}

func (settings *NativeSettings) Load() (Settings, error) {
	loaded, found, err := loadNativeSettings()
	if err != nil {
		return Settings{}, err
	}
	if !found {
		return Settings{}, ErrSettingsNotFound
	}
	return validateSettings(loaded)
}

func (settings *NativeSettings) Edit(current Settings) (Settings, bool, error) {
	validationMessage := ""
	for {
		candidate, accepted, err := promptNativeSettings(current, validationMessage)
		if err != nil || !accepted {
			return Settings{}, accepted, err
		}
		validated, validationErr := validateSettings(candidate)
		if validationErr != nil {
			current = candidate
			validationMessage = validationErr.Error()
			continue
		}
		if err := saveNativeSettings(validated); err != nil {
			return Settings{}, false, err
		}
		return validated, true, nil
	}
}

func validateSettings(settings Settings) (Settings, error) {
	settings.Server = strings.TrimSpace(settings.Server)
	settings.APIKey = strings.TrimSpace(settings.APIKey)
	if settings.Server == "" {
		return Settings{}, errors.New(localized("validation.server_required", "Please enter a Miniflux server URL."))
	}
	parsed, err := url.Parse(settings.Server)
	if err != nil || (parsed.Scheme != "http" && parsed.Scheme != "https") || parsed.Host == "" {
		return Settings{}, errors.New(localized("validation.server_invalid", "The server URL must be a complete HTTP or HTTPS URL."))
	}
	if settings.APIKey == "" {
		return Settings{}, errors.New(localized("validation.api_key_required", "Please enter a Miniflux API key."))
	}
	settings.Server = strings.TrimRight(settings.Server, "/")
	if settings.Server == "http:" || settings.Server == "https:" {
		return Settings{}, errors.New(localized("validation.server_invalid", "The server URL must be a complete HTTP or HTTPS URL."))
	}
	return settings, nil
}
