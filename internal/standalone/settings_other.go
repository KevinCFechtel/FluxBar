//go:build !darwin

package standalone

import "errors"

func loadNativeSettings() (Settings, bool, error) {
	return Settings{}, false, errors.New("der native Schlüsselbund ist nur unter macOS verfügbar")
}

func saveNativeSettings(Settings) error {
	return errors.New("der native Schlüsselbund ist nur unter macOS verfügbar")
}

func promptNativeSettings(Settings, string) (Settings, bool, error) {
	return Settings{}, false, errors.New("das native Einstellungsfenster ist nur unter macOS verfügbar")
}
