//go:build darwin

package standalone

/*
#cgo CFLAGS: -x objective-c -fblocks -fobjc-arc
#cgo LDFLAGS: -framework Cocoa -framework Security -framework ServiceManagement
#include <stdbool.h>
#include <stdlib.h>

int fluxbar_load_settings(
    char **server,
    char **api_key,
    bool *show_splash,
    bool *launch_at_login,
    bool *newest_first,
    char **error_message
);
bool fluxbar_save_settings(
    const char *server,
    const char *api_key,
    bool show_splash,
    bool launch_at_login,
    bool newest_first,
    char **error_message
);
int fluxbar_prompt_settings(
    const char *server,
    const char *api_key,
    bool show_splash,
    bool launch_at_login,
    bool newest_first,
    const char *validation_message,
    char **saved_server,
    char **saved_api_key,
    bool *saved_show_splash,
    bool *saved_launch_at_login,
    bool *saved_newest_first
);
*/
import "C"

import (
	"fmt"
	"unsafe"
)

func loadNativeSettings() (Settings, bool, error) {
	var server, apiKey, errorMessage *C.char
	var showSplash, launchAtLogin, newestFirst C.bool
	status := int(C.fluxbar_load_settings(
		&server,
		&apiKey,
		&showSplash,
		&launchAtLogin,
		&newestFirst,
		&errorMessage,
	))
	defer freeCString(server)
	defer freeCString(apiKey)
	defer freeCString(errorMessage)
	if status < 0 {
		return Settings{}, false, fmt.Errorf("%s", localizedFormat(
			"error.load_settings_format",
			"Load settings from the Keychain: %s",
			goString(errorMessage),
		))
	}
	if status == 0 {
		return Settings{}, false, nil
	}
	return Settings{
		Server:        goString(server),
		APIKey:        goString(apiKey),
		ShowSplash:    bool(showSplash),
		LaunchAtLogin: bool(launchAtLogin),
		NewestFirst:   bool(newestFirst),
	}, true, nil
}

func saveNativeSettings(settings Settings) error {
	server := C.CString(settings.Server)
	defer C.free(unsafe.Pointer(server))
	apiKey := C.CString(settings.APIKey)
	defer C.free(unsafe.Pointer(apiKey))
	var errorMessage *C.char
	defer func() { freeCString(errorMessage) }()
	if !bool(C.fluxbar_save_settings(
		server,
		apiKey,
		C.bool(settings.ShowSplash),
		C.bool(settings.LaunchAtLogin),
		C.bool(settings.NewestFirst),
		&errorMessage,
	)) {
		return fmt.Errorf("%s", localizedFormat(
			"error.save_settings_format",
			"Save settings to the Keychain: %s",
			goString(errorMessage),
		))
	}
	return nil
}

func promptNativeSettings(current Settings, validationMessage string) (Settings, bool, error) {
	server := C.CString(current.Server)
	defer C.free(unsafe.Pointer(server))
	apiKey := C.CString(current.APIKey)
	defer C.free(unsafe.Pointer(apiKey))
	message := C.CString(validationMessage)
	defer C.free(unsafe.Pointer(message))
	var savedServer, savedAPIKey *C.char
	var savedShowSplash, savedLaunchAtLogin, savedNewestFirst C.bool
	status := int(C.fluxbar_prompt_settings(
		server,
		apiKey,
		C.bool(current.ShowSplash),
		C.bool(current.LaunchAtLogin),
		C.bool(current.NewestFirst),
		message,
		&savedServer,
		&savedAPIKey,
		&savedShowSplash,
		&savedLaunchAtLogin,
		&savedNewestFirst,
	))
	defer freeCString(savedServer)
	defer freeCString(savedAPIKey)
	if status < 0 {
		return Settings{}, false, fmt.Errorf("%s", localized(
			"error.open_settings",
			"The settings window could not be opened.",
		))
	}
	if status == 0 {
		return Settings{}, false, nil
	}
	return Settings{
		Server:        goString(savedServer),
		APIKey:        goString(savedAPIKey),
		ShowSplash:    bool(savedShowSplash),
		LaunchAtLogin: bool(savedLaunchAtLogin),
		NewestFirst:   bool(savedNewestFirst),
	}, true, nil
}

func goString(value *C.char) string {
	if value == nil {
		return ""
	}
	return C.GoString(value)
}

func freeCString(value *C.char) {
	if value != nil {
		C.free(unsafe.Pointer(value))
	}
}
