//go:build darwin

package standalone

/*
#cgo CFLAGS: -x objective-c -fblocks -fobjc-arc
#cgo LDFLAGS: -framework Cocoa
#include <stdbool.h>
#include <stdlib.h>

bool fluxbar_initialize_article_hover(void);
void fluxbar_reset_article_hover(void);
void fluxbar_register_article_hover(
    const char *title,
    const char *feed,
    const char *preview,
    const char *image_url,
    const unsigned char *fallback_icon,
    int fallback_icon_length
);
void fluxbar_close_article_hover(void);
*/
import "C"

import (
	"unsafe"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

func initializeArticleHover() bool {
	return bool(C.fluxbar_initialize_article_hover())
}

func resetArticleHover() {
	C.fluxbar_reset_article_hover()
}

func registerArticleHover(entry model.Entry) {
	title := C.CString(entry.Title)
	defer C.free(unsafe.Pointer(title))
	feed := C.CString(entry.FeedName)
	defer C.free(unsafe.Pointer(feed))
	preview := C.CString(entry.Preview)
	defer C.free(unsafe.Pointer(preview))
	imageURL := C.CString(entry.ImageURL)
	defer C.free(unsafe.Pointer(imageURL))

	var iconPointer *C.uchar
	if len(entry.Icon) > 0 {
		iconPointer = (*C.uchar)(unsafe.Pointer(&entry.Icon[0]))
	}
	C.fluxbar_register_article_hover(
		title,
		feed,
		preview,
		imageURL,
		iconPointer,
		C.int(len(entry.Icon)),
	)
}

func closeArticleHover() {
	C.fluxbar_close_article_hover()
}
