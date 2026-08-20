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

DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}"
SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
CLANG="$(xcrun --sdk macosx --find clang)"
mkdir -p "${OUTPUT_DIR}"

archives=()
first_header=""
for arch in "${ARCHS[@]}"; do
  case "${arch}" in
    arm64) goarch="arm64" ;;
    x86_64) goarch="amd64" ;;
    *)
      echo "Nicht unterstützte macOS-Architektur: ${arch}" >&2
      exit 1
      ;;
  esac

  arch_dir="${OUTPUT_DIR}/${arch}"
  mkdir -p "${arch_dir}"
  (
    cd "${REPOSITORY_DIR}"
    env \
      CGO_ENABLED=1 \
      GOOS=darwin \
      GOARCH="${goarch}" \
      CC="${CLANG}" \
      SDKROOT="${SDK_PATH}" \
      MACOSX_DEPLOYMENT_TARGET="${DEPLOYMENT_TARGET}" \
      CGO_CFLAGS="-isysroot ${SDK_PATH} -mmacosx-version-min=${DEPLOYMENT_TARGET}" \
      CGO_LDFLAGS="-isysroot ${SDK_PATH} -mmacosx-version-min=${DEPLOYMENT_TARGET}" \
      go build -buildmode=c-archive -buildvcs=false -trimpath \
        -o "${arch_dir}/libfluxcore.a" ./cmd/fluxcore
  )
  archives+=("${arch_dir}/libfluxcore.a")
  if [[ -z "${first_header}" ]]; then
    first_header="${arch_dir}/libfluxcore.h"
  fi
done

if [[ "${#archives[@]}" -eq 1 ]]; then
  cp "${archives[0]}" "${OUTPUT_DIR}/libfluxcore.a"
else
  xcrun lipo -create "${archives[@]}" -output "${OUTPUT_DIR}/libfluxcore.a"
fi
cp "${first_header}" "${OUTPUT_DIR}/libfluxcore.h"
