//go:build darwin

package standalone

/*
#cgo CFLAGS: -x objective-c -fblocks -fobjc-arc
#cgo LDFLAGS: -framework Cocoa

void fluxbar_show_startup_splash(void);
*/
import "C"

func showStartupSplash() {
	C.fluxbar_show_startup_splash()
}
