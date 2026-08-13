//go:build darwin

package standalone

/*
#cgo CFLAGS: -x objective-c -fblocks -fobjc-arc
#cgo LDFLAGS: -framework Cocoa -framework Security
#include <stdbool.h>
#include <stdlib.h>

int fluxbar_load_settings(char **server, char **api_key, bool *show_splash, char **error_message);
bool fluxbar_save_settings(const char *server, const char *api_key, bool show_splash, char **error_message);
int fluxbar_prompt_settings(
    const char *server,
    const char *api_key,
    bool show_splash,
    const char *validation_message,
    char **saved_server,
    char **saved_api_key,
    bool *saved_show_splash
);
*/
import "C"

import (
	"fmt"
	"unsafe"
)

func loadNativeSettings() (Settings, bool, error) {
	var server, apiKey, errorMessage *C.char
	var showSplash C.bool
	status := int(C.fluxbar_load_settings(&server, &apiKey, &showSplash, &errorMessage))
	defer freeCString(server)
	defer freeCString(apiKey)
	defer freeCString(errorMessage)
	if status < 0 {
		return Settings{}, false, fmt.Errorf("Einstellungen aus dem Schlüsselbund laden: %s", goString(errorMessage))
	}
	if status == 0 {
		return Settings{}, false, nil
	}
	return Settings{Server: goString(server), APIKey: goString(apiKey), ShowSplash: bool(showSplash)}, true, nil
}

func saveNativeSettings(settings Settings) error {
	server := C.CString(settings.Server)
	defer C.free(unsafe.Pointer(server))
	apiKey := C.CString(settings.APIKey)
	defer C.free(unsafe.Pointer(apiKey))
	var errorMessage *C.char
	defer func() { freeCString(errorMessage) }()
	if !bool(C.fluxbar_save_settings(server, apiKey, C.bool(settings.ShowSplash), &errorMessage)) {
		return fmt.Errorf("Einstellungen im Schlüsselbund speichern: %s", goString(errorMessage))
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
	var savedShowSplash C.bool
	status := int(C.fluxbar_prompt_settings(
		server,
		apiKey,
		C.bool(current.ShowSplash),
		message,
		&savedServer,
		&savedAPIKey,
		&savedShowSplash,
	))
	defer freeCString(savedServer)
	defer freeCString(savedAPIKey)
	if status < 0 {
		return Settings{}, false, fmt.Errorf("Einstellungsfenster konnte nicht geöffnet werden")
	}
	if status == 0 {
		return Settings{}, false, nil
	}
	return Settings{
		Server:     goString(savedServer),
		APIKey:     goString(savedAPIKey),
		ShowSplash: bool(savedShowSplash),
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
