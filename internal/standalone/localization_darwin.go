//go:build darwin

package standalone

/*
#include <stdlib.h>
*/
import "C"

// FluxBarCopyLocalizedString exposes the shared Go localization layer to the
// native AppKit files. The caller owns the returned C string.
//
//export FluxBarCopyLocalizedString
func FluxBarCopyLocalizedString(keyValue, fallbackValue *C.char) *C.char {
	return C.CString(localized(C.GoString(keyValue), C.GoString(fallbackValue)))
}
