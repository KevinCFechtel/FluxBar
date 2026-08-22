#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${1:?output directory required}"
shift
ARCHS=("$@")
DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-15.0}"

if [[ "${#ARCHS[@]}" -eq 0 ]]; then
  echo "Mindestens eine Architektur ist erforderlich." >&2
  exit 1
fi

targets=()
for arch in "${ARCHS[@]}"; do
  case "${arch}" in
    arm64)
      targets+=("aarch64-apple-darwin")
      ;;
    x86_64)
      targets+=("x86_64-apple-darwin")
      ;;
    *)
      echo "Nicht unterstützte macOS-Architektur: ${arch}" >&2
      exit 1
      ;;
  esac
done

if ! command -v rustup >/dev/null 2>&1; then
  echo "Benötigtes Programm fehlt: rustup" >&2
  echo "Die installierten Rust-Ziele können nicht geprüft werden." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Benötigtes Programm fehlt: cargo" >&2
  exit 1
fi

if ! installed_targets="$(rustup target list --installed)"; then
  echo "Installierte Rust-Ziele konnten nicht ermittelt werden." >&2
  exit 1
fi

missing_targets=()
for target in "${targets[@]}"; do
  if ! grep -Fqx -- "${target}" <<< "${installed_targets}"; then
    missing_targets+=("${target}")
  fi
done

if [[ "${#missing_targets[@]}" -ne 0 ]]; then
  for target in "${missing_targets[@]}"; do
    echo "Erforderliches Rust-Ziel ist nicht installiert: ${target}" >&2
    echo "Installieren mit: rustup target add ${target}" >&2
  done
  echo "Das Build-Skript verändert die Rust-Toolchain nicht." >&2
  exit 1
fi

CLANG="$(xcrun --sdk macosx --find clang)"
SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"

verify_archive() {
  local arch="$1"
  local archive="$2"

  if ! xcrun lipo "${archive}" -verify_arch "${arch}" >/dev/null; then
    echo "Rust-Archiv enthält nicht die erwartete Architektur ${arch}: ${archive}" >&2
    exit 1
  fi
}

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
    if (strstr(r2, "Miniflux is not configured") == NULL) {
        fprintf(stderr, "unexpected op response: %s\n", r2);
        return 1;
    }
    FluxCoreFree(r2);

    return 0;
}
EOF

mkdir -p "${OUTPUT_DIR}"

archives=()
for index in "${!ARCHS[@]}"; do
  arch="${ARCHS[${index}]}"
  target="${targets[${index}]}"

  arch_dir="${OUTPUT_DIR}/${arch}"
  mkdir -p "${arch_dir}"

  MACOSX_DEPLOYMENT_TARGET="${DEPLOYMENT_TARGET}" cargo build \
    --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
    --target "${target}" \
    --release

  cp "${REPOSITORY_DIR}/rust-core/target/${target}/release/libfluxcore.a" \
    "${arch_dir}/libfluxcore.a"
  verify_archive "${arch}" "${arch_dir}/libfluxcore.a"

  "${CLANG}" -arch "${arch}" -isysroot "${SDK_PATH}" \
    -mmacosx-version-min="${DEPLOYMENT_TARGET}" \
    -o "${SMOKE_DIR}/smoke-${arch}" "${SMOKE_DIR}/smoke.c" \
    "${arch_dir}/libfluxcore.a" \
    -framework CoreFoundation -framework Security

  if [[ "$(uname -m)" == "${arch}" ]]; then
    "${SMOKE_DIR}/smoke-${arch}"
  fi

  archives+=("${arch_dir}/libfluxcore.a")
done

if [[ "${#archives[@]}" -eq 1 ]]; then
  cp "${archives[0]}" "${OUTPUT_DIR}/libfluxcore.a"
else
  xcrun lipo -create "${archives[@]}" -output "${OUTPUT_DIR}/libfluxcore.a"
fi

cp "${REPOSITORY_DIR}/rust-core/libfluxcore.h" "${OUTPUT_DIR}/libfluxcore.h"

echo "Rust core static library: ${OUTPUT_DIR}/libfluxcore.a"
