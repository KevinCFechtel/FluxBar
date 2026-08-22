#ifndef FLUXCORE_H
#define FLUXCORE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Minimal FluxBar core ABI.
 *
 * This header declares the same C boundary used by the Go core so that the
 * native macOS client can link either implementation without source changes.
 *
 * All responses are core-owned UTF-8 C strings. The caller must release a
 * non-null pointer returned by FluxCoreRequest by calling FluxCoreFree.
 */

/*
 * Processes a JSON request and returns a core-owned null-terminated response.
 *
 * request may be NULL. The pointer is borrowed for the duration of the call
 * and is not retained by the core.
 */
extern char* FluxCoreRequest(char* request);

/*
 * Releases a response string previously returned by FluxCoreRequest.
 *
 * value may be NULL, in which case this function is a no-op.
 */
extern void FluxCoreFree(char* value);

#ifdef __cplusplus
}
#endif

#endif /* FLUXCORE_H */
