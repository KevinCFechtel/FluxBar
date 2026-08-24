#!/usr/bin/env bash
set -euo pipefail

# Build raw Android static archives for the Rust core.
#
# Usage:
#   Build/build-rust-android.sh [output-dir] [profile]
#
# Defaults:
#   output-dir = .build/mobile/android
#   profile    = release
#
# Environment:
#   ANDROID_HOME                 - Android SDK root (required if NDK not given)
#   FLUX_ANDROID_NDK             - NDK root path (optional; default derived)
#   FLUX_ANDROID_API             - NDK API level for linkers (default 29)
#   CARGO_FEATURES               - comma-separated features passed to cargo build
#
# The script validates tools and targets, prints exact remediation, and never
# installs Rust targets, SDK/NDK components, or other toolchain parts automatically.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${1:-${REPOSITORY_DIR}/.build/mobile/android}"
PROFILE="${2:-release}"
API_LEVEL="${FLUX_ANDROID_API:-29}"

REQUIRED_TARGETS=(
  "aarch64-linux-android"
  "x86_64-linux-android"
  "armv7-linux-androideabi"
)

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

NDK_ROOT="${FLUX_ANDROID_NDK:-}"
if [[ -z "${NDK_ROOT}" ]]; then
  if [[ -z "${ANDROID_HOME:-}" ]]; then
    fail "ANDROID_HOME is not set and FLUX_ANDROID_NDK was not provided."
  fi
  # Prefer the NDK version pinned by the audited FluxNews configuration.
  pinned_ndk="${ANDROID_HOME}/ndk/28.2.13676358"
  if [[ -d "${pinned_ndk}" ]]; then
    NDK_ROOT="${pinned_ndk}"
  elif [[ -d "${ANDROID_HOME}/ndk" ]]; then
    # Fall back to the newest installed NDK directory.
    NDK_ROOT="$(find "${ANDROID_HOME}/ndk" -maxdepth 1 -type d | sort | tail -1)"
  fi
fi

if [[ -z "${NDK_ROOT}" || ! -d "${NDK_ROOT}" ]]; then
  fail "Android NDK not found. Set FLUX_ANDROID_NDK or install the NDK under ANDROID_HOME/ndk."
fi

TOOLCHAIN="${NDK_ROOT}/toolchains/llvm/prebuilt/darwin-x86_64"
if [[ ! -d "${TOOLCHAIN}" ]]; then
  fail "NDK LLVM toolchain not found at ${TOOLCHAIN}."
fi

BIN_DIR="${TOOLCHAIN}/bin"

clang_for_target() {
  local target="$1"
  case "${target}" in
    aarch64-linux-android)
      echo "${BIN_DIR}/aarch64-linux-android${API_LEVEL}-clang"
      ;;
    x86_64-linux-android)
      echo "${BIN_DIR}/x86_64-linux-android${API_LEVEL}-clang"
      ;;
    armv7-linux-androideabi)
      echo "${BIN_DIR}/armv7a-linux-androideabi${API_LEVEL}-clang"
      ;;
    *)
      fail "Unknown Android target: ${target}"
      ;;
  esac
}

for target in "${REQUIRED_TARGETS[@]}"; do
  clang="$(clang_for_target "${target}")"
  if [[ ! -x "${clang}" ]]; then
    fail "Missing NDK clang for ${target}: ${clang}"
  fi
done

if [[ ! -x "${BIN_DIR}/llvm-ar" ]]; then
  fail "Missing NDK llvm-ar at ${BIN_DIR}/llvm-ar"
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

  local clang="$(clang_for_target "${target}")"
  local linker_var="CARGO_TARGET_$(echo "${target}" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  local target_underscores="$(echo "${target}" | tr '-' '_')"
  local cc_var="CC_${target_underscores}"
  local ar_var="AR_${target_underscores}"

  local start_epoch="$(date +%s)"
  echo "Building ${target} (${PROFILE})..."
  export "${linker_var}=${clang}"
  export "${cc_var}=${clang}"
  export "${ar_var}=${BIN_DIR}/llvm-ar"

  local feature_args=()
  if [[ -n "${CARGO_FEATURES:-}" ]]; then
    feature_args+=(--features "${CARGO_FEATURES}")
  fi

  if [[ ${#feature_args[@]} -gt 0 ]]; then
    cargo build \
      --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
      --target "${target}" \
      --profile "${PROFILE}" \
      "${feature_args[@]}"
  else
    cargo build \
      --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
      --target "${target}" \
      --profile "${PROFILE}"
  fi
  local end_epoch="$(date +%s)"

  local src_archive="${REPOSITORY_DIR}/rust-core/target/${target}/${PROFILE}/libfluxcore.a"
  local dst_archive="${target_dir}/libfluxcore.a"
  cp "${src_archive}" "${dst_archive}"

  symbols=$("${RUST_LLVM_NM}" "${dst_archive}" 2>/dev/null | grep -E 'FluxCore(Request|Free)' || true)
  if [[ -z "${symbols}" ]]; then
    fail "Archive ${dst_archive} is missing exported FluxCore symbols"
  fi

  local readelf_machine
  case "${target}" in
    aarch64-linux-android) readelf_machine="AArch64" ;;
    x86_64-linux-android) readelf_machine="X86-64" ;;
    armv7-linux-androideabi) readelf_machine="ARM" ;;
  esac

  machine_lines=$("${BIN_DIR}/llvm-readelf" -h "${dst_archive}" 2>/dev/null | grep "Machine:.*${readelf_machine}" || true)
  if [[ -z "${machine_lines}" ]]; then
    fail "Archive ${dst_archive} does not have expected ELF machine ${readelf_machine}"
  fi

  local size="$(stat -f%z "${dst_archive}" 2>/dev/null || stat -c%s "${dst_archive}" 2>/dev/null)"
  local sha256="$(shasum -a 256 "${dst_archive}" | awk '{print $1}')"
  local duration=$((end_epoch - start_epoch))

  manifest_entries+=("    {\"target\": \"${target}\", \"abi\": \"${arch_label}\", \"size\": ${size}, \"sha256\": \"${sha256}\", \"durationSeconds\": ${duration}}")
}

rm -rf "${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}"

build_target "aarch64-linux-android" "arm64-v8a" "arm64-v8a"
build_target "x86_64-linux-android" "x86_64" "x86_64"
build_target "armv7-linux-androideabi" "armeabi-v7a" "armeabi-v7a"

mkdir -p "${OUTPUT_DIR}/Headers"
cp "${REPOSITORY_DIR}/rust-core/libfluxcore.h" "${OUTPUT_DIR}/Headers/libfluxcore.h"
cat > "${OUTPUT_DIR}/Headers/module.modulemap" <<'EOF'
module FluxCore {
    header "libfluxcore.h"
    export *
}
EOF

rust_version="$(rustc --version)"
cargo_version="$(cargo --version)"
ndk_version="$(cat "${NDK_ROOT}/source.properties" 2>/dev/null | grep 'Pkg.Revision' | awk -F= '{print $2}' | tr -d ' ' || echo 'unknown')"

manifest_entries_joined="$(IFS=,; echo "${manifest_entries[*]}")"

mkdir -p "$(dirname "${OUTPUT_DIR}/manifest.json")"
cat > "${OUTPUT_DIR}/manifest.json" <<EOF
{
  "platform": "android",
  "rustVersion": "${rust_version}",
  "cargoVersion": "${cargo_version}",
  "ndkRoot": "${NDK_ROOT}",
  "ndkVersion": "${ndk_version}",
  "apiLevel": ${API_LEVEL},
  "profile": "${PROFILE}",
  "buildStarted": "${build_start_global}",
  "buildFinished": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "targets": [
${manifest_entries_joined}
  ]
}
EOF

echo "Android Rust core archives: ${OUTPUT_DIR}"
