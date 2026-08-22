#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${1:?output directory required}"

mkdir -p "${OUTPUT_DIR}"

cargo build --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" --release

ARTIFACT="${REPOSITORY_DIR}/rust-core/target/release/librustcore.a"
cp "${ARTIFACT}" "${OUTPUT_DIR}/librustcore.a"

# Smoke-test the produced static library by compiling and running a tiny C
# caller. This proves the expected C symbols are exported and callable.
SMOKE_DIR="$(mktemp -d)"
trap 'rm -rf "${SMOKE_DIR}"' EXIT

cat > "${SMOKE_DIR}/smoke.c" <<'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern char* FluxCoreRequest(char* request);
extern void FluxCoreFree(char* value);

int main(void) {
    char* r1 = FluxCoreRequest(NULL);
    if (strcmp(r1, "{\"ok\":false,\"error\":\"null request\"}") != 0) {
        fprintf(stderr, "unexpected null response: %s\n", r1);
        return 1;
    }
    FluxCoreFree(r1);

    char req[] = "{\"operation\":\"refresh\"}";
    char* r2 = FluxCoreRequest(req);
    if (strstr(r2, "not implemented: refresh") == NULL) {
        fprintf(stderr, "unexpected op response: %s\n", r2);
        return 1;
    }
    FluxCoreFree(r2);

    return 0;
}
EOF

cc -o "${SMOKE_DIR}/smoke" "${SMOKE_DIR}/smoke.c" "${OUTPUT_DIR}/librustcore.a"
"${SMOKE_DIR}/smoke"

echo "Rust core static library: ${OUTPUT_DIR}/librustcore.a"
