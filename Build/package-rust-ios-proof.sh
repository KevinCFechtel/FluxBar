#!/usr/bin/env bash
set -euo pipefail

# Package pre-built iOS Rust static archives into an XCFramework for the
# mobile-runtime-proof host. This is the bridge between
# `Build/build-rust-ios.sh` and the Xcode project.
#
# Usage:
#   Build/package-rust-ios-proof.sh [ios-build-dir] [output-xcframework]
#
# Defaults:
#   ios-build-dir      = .build/mobile/ios
#   output-xcframework = .build/mobile/ios/FluxCore.xcframework
#
# The script supports arm64 device and arm64 simulator archives. An optional
# x86_64 simulator archive is included when BUILD_INTEL_SIMULATOR=1.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

INPUT_DIR="${1:-${REPOSITORY_DIR}/.build/mobile/ios}"
OUTPUT_XCFRAMEWORK="${2:-${INPUT_DIR}/FluxCore.xcframework}"

fail() {
  echo "$1" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "Required command not found: $1"
  fi
}

require_command xcodebuild

DEVICE_ARCHIVE="${INPUT_DIR}/device-arm64/libfluxcore.a"
SIM_ARM64_ARCHIVE="${INPUT_DIR}/simulator-arm64/libfluxcore.a"
HEADER_DIR="${INPUT_DIR}/Headers"
HEADER="${HEADER_DIR}/libfluxcore.h"
MODULE_MAP="${HEADER_DIR}/module.modulemap"

[[ -f "${DEVICE_ARCHIVE}" ]] || fail "Missing device archive: ${DEVICE_ARCHIVE}"
[[ -f "${SIM_ARM64_ARCHIVE}" ]] || fail "Missing arm64 simulator archive: ${SIM_ARM64_ARCHIVE}"
[[ -f "${HEADER}" ]] || fail "Missing C header: ${HEADER}"

# Swift can import C functions from a static-library XCFramework only when a
# module map is provided. xcodebuild -create-xcframework does not generate one
# for static libraries, so we provide our own.
cat > "${MODULE_MAP}" <<'EOF'
module FluxCore {
    header "libfluxcore.h"
    export *
}
EOF

rm -rf "${OUTPUT_XCFRAMEWORK}"

xcodebuild_args=(
  -create-xcframework
  -library "${DEVICE_ARCHIVE}"
  -headers "${HEADER_DIR}"
  -library "${SIM_ARM64_ARCHIVE}"
  -headers "${HEADER_DIR}"
)

if [[ "${BUILD_INTEL_SIMULATOR:-0}" == "1" ]]; then
  SIM_X86_64_ARCHIVE="${INPUT_DIR}/simulator-x86_64/libfluxcore.a"
  [[ -f "${SIM_X86_64_ARCHIVE}" ]] || fail "Missing x86_64 simulator archive: ${SIM_X86_64_ARCHIVE}"
  xcodebuild_args+=(-library "${SIM_X86_64_ARCHIVE}" -headers "${HEADER_DIR}")
fi

xcodebuild_args+=(-output "${OUTPUT_XCFRAMEWORK}")

xcodebuild "${xcodebuild_args[@]}"

echo "XCFramework: ${OUTPUT_XCFRAMEWORK}"
