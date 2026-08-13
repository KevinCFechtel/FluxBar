//go:build darwin

package standalone

/*
#cgo CFLAGS: -x objective-c -fblocks -fobjc-arc
#cgo LDFLAGS: -framework Cocoa
#include <stdbool.h>
#include <stdint.h>

bool fluxbar_is_dark_appearance(void);
bool fluxbar_start_appearance_observation(uintptr_t context);
void fluxbar_stop_appearance_observation(void);
*/
import "C"

import "runtime/cgo"

func darkAppearance() bool {
	return bool(C.fluxbar_is_dark_appearance())
}

func observeAppearance(notify func(bool)) (func(), bool) {
	handle := cgo.NewHandle(notify)
	if !bool(C.fluxbar_start_appearance_observation(C.uintptr_t(handle))) {
		handle.Delete()
		return func() {}, false
	}
	return func() {
		C.fluxbar_stop_appearance_observation()
		handle.Delete()
	}, true
}

//export fluxbar_appearance_changed
func fluxbar_appearance_changed(context C.uintptr_t, dark C.bool) {
	notify, ok := cgo.Handle(context).Value().(func(bool))
	if ok {
		notify(bool(dark))
	}
}
