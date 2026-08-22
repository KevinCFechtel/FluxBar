#!/usr/bin/env bash
set -euo pipefail

# Transport-level compatibility smoke test for the Go and Rust cores.
#
# Builds both cores, links a tiny C caller against each, and compares the
# outputs for cases that are expected to match in Phase 3. Domain-level
# operations are intentionally not compared because Rust handlers remain
# unimplemented.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

GO_DIR="${WORK_DIR}/go"
RUST_DIR="${WORK_DIR}/rust"

mkdir -p "${GO_DIR}" "${RUST_DIR}"

echo "Building Go core..."
"${SCRIPT_DIR}/build-go-core.sh" "${GO_DIR}" arm64

echo "Building Rust core..."
"${SCRIPT_DIR}/build-rust-core.sh" "${RUST_DIR}" arm64

cat > "${WORK_DIR}/caller.c" <<'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern char* FluxCoreRequest(char* request);
extern void FluxCoreFree(char* value);

static void call(const char* label, const char* request) {
    char* r = FluxCoreRequest((char*)request);
    printf("%s: %s\n", label, r ? r : "(null)");
    FluxCoreFree(r);
}

int main(void) {
    call("null", NULL);
    call("malformed", "not-json");
    call("missing_operation", "{}");
    call("unknown_operation", "{\"operation\":\"unknown\"}");
    return 0;
}
EOF

cc -o "${WORK_DIR}/caller-go" "${WORK_DIR}/caller.c" "${GO_DIR}/libfluxcore.a" \
  -framework CoreFoundation -framework Security -lresolv
cc -o "${WORK_DIR}/caller-rust" "${WORK_DIR}/caller.c" "${RUST_DIR}/libfluxcore.a"

echo ""
echo "=== Go core ==="
"${WORK_DIR}/caller-go"

echo ""
echo "=== Rust core ==="
"${WORK_DIR}/caller-rust"

echo ""
echo "Transport compatibility smoke test completed."
