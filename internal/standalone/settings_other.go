//go:build !darwin

package standalone

import "errors"

func loadNativeSettings() (Settings, bool, error) {
	return Settings{}, false, errors.New(localized(
		"error.secure_storage_macos_only",
		"Native secure credential storage is only available on macOS.",
	))
}

func saveNativeSettings(Settings) error {
	return errors.New(localized(
		"error.secure_storage_macos_only",
		"Native secure credential storage is only available on macOS.",
	))
}

func promptNativeSettings(Settings, string) (Settings, bool, error) {
	return Settings{}, false, errors.New(localized(
		"error.settings_window_macos_only",
		"The native settings window is only available on macOS.",
	))
}
