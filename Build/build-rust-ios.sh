#!/usr/bin/env bash
set -euo pipefail

# Build raw iOS static archives for the Rust core.
#
# Usage:
#   Build/build-rust-ios.sh [output-dir] [profile]
#
# Defaults:
#   output-dir = .build/mobile/ios
#   profile    = release
#
# Environment:
#   IPHONEOS_DEPLOYMENT_TARGET - iOS deployment target (default 17.0)
#   CARGO_FEATURES             - comma-separated features passed to cargo build
#
# The script validates tools and targets, prints exact remediation, and never
# installs Rust targets, Xcode SDKs, or other toolchain components automatically.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${1:-${REPOSITORY_DIR}/.build/mobile/ios}"
PROFILE="${2:-release}"
DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"

REQUIRED_TARGETS=(
  "aarch64-apple-ios"
  "aarch64-apple-ios-sim"
)

BUILD_INTEL_SIMULATOR="${BUILD_INTEL_SIMULATOR:-0}"
if [[ "${BUILD_INTEL_SIMULATOR}" == "1" ]]; then
  REQUIRED_TARGETS+=("x86_64-apple-ios")
fi

fail() {
  echo "$1" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "Required command not found: $1"
  fi
}

require_command rustup
require_command cargo
require_command xcrun

if ! installed_targets="$(rustup target list --installed)"; then
  fail "Could not list installed Rust targets."
fi

missing_targets=()
for target in "${REQUIRED_TARGETS[@]}"; do
  if ! grep -Fqx -- "${target}" <<< "${installed_targets}"; then
    missing_targets+=("${target}")
  fi
done

if [[ "${#missing_targets[@]}" -ne 0 ]]; then
  for target in "${missing_targets[@]}"; do
    echo "Missing required Rust target: ${target}" >&2
    echo "Install with: rustup target add ${target}" >&2
  done
  fail "This script does not modify the Rust toolchain."
fi

if ! xcrun --sdk iphoneos --show-sdk-path >/dev/null 2>&1; then
  fail "iOS device SDK (iphoneos) not found. Ensure Xcode is installed and includes the iOS SDK."
fi

if ! xcrun --sdk iphonesimulator --show-sdk-path >/dev/null 2>&1; then
  fail "iOS simulator SDK (iphonesimulator) not found. Ensure Xcode is installed and includes the iOS Simulator SDK."
fi

RUST_LLVM_NM="${REPOSITORY_DIR}/rust-core/target/rust-llvm-nm"
if [[ ! -x "${RUST_LLVM_NM}" ]]; then
  rust_sysroot="$(rustc --print sysroot)"
  candidate="${rust_sysroot}/lib/rustlib/aarch64-apple-darwin/bin/llvm-nm"
  if [[ -x "${candidate}" ]]; then
    RUST_LLVM_NM="${candidate}"
  else
    fail "Could not locate Rust llvm-nm at ${candidate}."
  fi
fi

manifest_entries=()
build_start_global="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

build_target() {
  local target="$1"
  local output_subdir="$2"
  local arch_label="$3"

  local target_dir="${OUTPUT_DIR}/${output_subdir}"
  mkdir -p "${target_dir}"

  local start_epoch="$(date +%s)"
  echo "Building ${target} (${PROFILE})..."
  local feature_args=()
  if [[ -n "${CARGO_FEATURES:-}" ]]; then
    feature_args+=(--features "${CARGO_FEATURES}")
  fi

  if [[ ${#feature_args[@]} -gt 0 ]]; then
    IPHONEOS_DEPLOYMENT_TARGET="${DEPLOYMENT_TARGET}" \
      cargo build \
        --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
        --target "${target}" \
        --profile "${PROFILE}" \
        "${feature_args[@]}"
  else
    IPHONEOS_DEPLOYMENT_TARGET="${DEPLOYMENT_TARGET}" \
      cargo build \
        --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
        --target "${target}" \
        --profile "${PROFILE}"
  fi
  local end_epoch="$(date +%s)"

  local src_archive="${REPOSITORY_DIR}/rust-core/target/${target}/${PROFILE}/libfluxcore.a"
  local dst_archive="${target_dir}/libfluxcore.a"
  cp "${src_archive}" "${dst_archive}"

  local lipo_arch
  case "${target}" in
    aarch64-apple-ios) lipo_arch="arm64" ;;
    aarch64-apple-ios-sim) lipo_arch="arm64" ;;
    x86_64-apple-ios) lipo_arch="x86_64" ;;
    *) lipo_arch="${target}" ;;
  esac

  if ! xcrun lipo "${dst_archive}" -verify_arch "${lipo_arch}" >/dev/null; then
    fail "Archive ${dst_archive} does not contain expected architecture ${lipo_arch}"
  fi

  symbols=$("${RUST_LLVM_NM}" "${dst_archive}" 2>/dev/null | grep -E '_FluxCore(Request|Free)' || true)
  if [[ -z "${symbols}" ]]; then
    fail "Archive ${dst_archive} is missing exported FluxCore symbols"
  fi

  local size="$(stat -f%z "${dst_archive}" 2>/dev/null || stat -c%s "${dst_archive}" 2>/dev/null)"
  local sha256="$(shasum -a 256 "${dst_archive}" | awk '{print $1}')"
  local duration=$((end_epoch - start_epoch))

  manifest_targets+=("${target}")
  manifest_entries+=("    {\"target\": \"${target}\", \"abi\": \"${arch_label}\", \"size\": ${size}, \"sha256\": \"${sha256}\", \"durationSeconds\": ${duration}}")
}

rm -rf "${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}"

build_target "aarch64-apple-ios" "device-arm64" "arm64"
build_target "aarch64-apple-ios-sim" "simulator-arm64" "arm64"
if [[ "${BUILD_INTEL_SIMULATOR}" == "1" ]]; then
  build_target "x86_64-apple-ios" "simulator-x86_64" "x86_64"
fi

mkdir -p "${OUTPUT_DIR}/Headers"
cp "${REPOSITORY_DIR}/rust-core/libfluxcore.h" "${OUTPUT_DIR}/Headers/libfluxcore.h"

rust_version="$(rustc --version)"
cargo_version=""  # populated below if available
cargo_version="$(cargo --version)"
xcode_version_tmp="$(mktemp)"
xcodebuild -version > "${xcode_version_tmp}"
xcode_version="$(head -1 "${xcode_version_tmp}")"
rm -f "${xcode_version_tmp}"
iphoneos_sdk="$(xcrun --sdk iphoneos --show-sdk-version)"
iphonesimulator_sdk="$(xcrun --sdk iphonesimulator --show-sdk-version)"

manifest_entries_joined="$(IFS=,; echo "${manifest_entries[*]}")"

mkdir -p "$(dirname "${OUTPUT_DIR}/manifest.json")"
cat > "${OUTPUT_DIR}/manifest.json" <<EOF
{
  "platform": "ios",
  "rustVersion": "${rust_version}",
  "cargoVersion": "${cargo_version}",
  "xcodeVersion": "${xcode_version}",
  "iphoneosSdk": "${iphoneos_sdk}",
  "iphonesimulatorSdk": "${iphonesimulator_sdk}",
  "deploymentTarget": "${DEPLOYMENT_TARGET}",
  "profile": "${PROFILE}",
  "buildStarted": "${build_start_global}",
  "buildFinished": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "targets": [
${manifest_entries_joined}
  ]
}
EOF

echo "iOS Rust core archives: ${OUTPUT_DIR}"
