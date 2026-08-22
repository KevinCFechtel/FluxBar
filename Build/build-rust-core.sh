#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${1:?output directory required}"
shift
ARCHS=("$@")

if [[ "${#ARCHS[@]}" -eq 0 ]]; then
  echo "Mindestens eine Architektur ist erforderlich." >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"

# Verify that the requested Rust target is installed. The build script does
# not modify the developer or CI toolchain automatically.
require_target() {
  local target="$1"
  if ! rustup target list --installed | grep -qx "${target}"; then
    echo "Erforderliches Rust-Ziel ist nicht installiert: ${target}" >&2
    echo "" >&2
    echo "Installieren mit:" >&2
    echo "    rustup target add ${target}" >&2
    exit 1
  fi
}

archives=()
for arch in "${ARCHS[@]}"; do
  case "${arch}" in
    arm64)
      target="aarch64-apple-darwin"
      ;;
    x86_64)
      target="x86_64-apple-darwin"
      ;;
    *)
      echo "Nicht unterstützte macOS-Architektur: ${arch}" >&2
      exit 1
      ;;
  esac

  require_target "${target}"

  arch_dir="${OUTPUT_DIR}/${arch}"
  mkdir -p "${arch_dir}"

  cargo build \
    --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
    --target "${target}" \
    --release

  cp "${REPOSITORY_DIR}/rust-core/target/${target}/release/libfluxcore.a" \
    "${arch_dir}/libfluxcore.a"
  archives+=("${arch_dir}/libfluxcore.a")
done

if [[ "${#archives[@]}" -eq 1 ]]; then
  cp "${archives[0]}" "${OUTPUT_DIR}/libfluxcore.a"
else
  xcrun lipo -create "${archives[@]}" -output "${OUTPUT_DIR}/libfluxcore.a"
fi

cp "${REPOSITORY_DIR}/rust-core/libfluxcore.h" "${OUTPUT_DIR}/libfluxcore.h"

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

cc -o "${SMOKE_DIR}/smoke" "${SMOKE_DIR}/smoke.c" "${OUTPUT_DIR}/libfluxcore.a"
"${SMOKE_DIR}/smoke"

echo "Rust core static library: ${OUTPUT_DIR}/libfluxcore.a"
