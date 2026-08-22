package main

/*
#include <stdlib.h>
*/
import "C"

import (
	"unsafe"

	"github.com/KevinCFechtel/FluxBar/internal/coreapi"
)

var runtime = coreapi.New(nil)

//export FluxCoreRequest
func FluxCoreRequest(request *C.char) *C.char {
	if request == nil {
		return C.CString(`{"ok":false,"error":"null request"}`)
	}
	return C.CString(runtime.HandleJSON(C.GoString(request)))
}

//export FluxCoreFree
func FluxCoreFree(value *C.char) {
	if value != nil {
		C.free(unsafe.Pointer(value))
	}
}

func main() {}
