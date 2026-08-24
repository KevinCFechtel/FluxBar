#!/usr/bin/env bash
set -euo pipefail

# Build raw Android static archives for the Rust core. When
# CARGO_FEATURES includes "mobile-runtime-proof", this script also builds
# the host-loadable libfluxcore_mobile_probe.so JNI shim, packages the
# pinned rustls-platform-verifier Android AAR, verifies ELF symbols and
# dependencies, and writes a combined manifest.
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
so_manifest_entries=()
build_start_global="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

BUILDING_PROOF=0
case ",${CARGO_FEATURES:-}," in
  *,mobile-runtime-proof,*)
    BUILDING_PROOF=1
    ;;
esac

# Locate the pinned rustls-platform-verifier-android AAR via cargo metadata.
# This keeps the Gradle dependency and the packaged AAR in sync with Cargo.lock.
locate_verifier_aar() {
  local metadata
  if ! metadata="$(cargo metadata --format-version 1 --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" --filter-platform "aarch64-linux-android" 2>/dev/null)"; then
    fail "cargo metadata failed; cannot locate rustls-platform-verifier-android AAR"
  fi
  local manifest_path
  manifest_path="$(printf '%s' "${metadata}" | python3 -c 'import json,sys; print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rustls-platform-verifier-android"))')" || \
    fail "rustls-platform-verifier-android package not found in cargo metadata"
  local aar="${manifest_path%/*}/maven/rustls/rustls-platform-verifier/0.1.1/rustls-platform-verifier-0.1.1.aar"
  if [[ ! -f "${aar}" ]]; then
    fail "rustls-platform-verifier AAR not found at ${aar}"
  fi
  printf '%s' "${aar}"
}

require_cmake() {
  if command -v cmake >/dev/null 2>&1; then
    CMAKE=cmake
    return
  fi
  # Android SDK bundles CMake under $ANDROID_HOME/cmake/<version>/bin/cmake.
  if [[ -n "${ANDROID_HOME:-}" ]]; then
    local sdk_cmake
    sdk_cmake="$(find "${ANDROID_HOME}/cmake" -maxdepth 3 -name cmake -type f -perm +111 2>/dev/null | head -1)"
    if [[ -n "${sdk_cmake}" ]]; then
      CMAKE="${sdk_cmake}"
      return
    fi
  fi
  fail "CMake is required for JNI .so packaging but was not found. Set PATH to include cmake or install the CMake SDK component."
}

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
    x86_64-linux-android) readelf_machine="X86-64|Advanced Micro Devices X86-64" ;;
    armv7-linux-androideabi) readelf_machine="ARM" ;;
  esac

  machine_lines=$("${BIN_DIR}/llvm-readelf" -h "${dst_archive}" 2>/dev/null | grep -E "Machine:.*(${readelf_machine})" || true)
  if [[ -z "${machine_lines}" ]]; then
    fail "Archive ${dst_archive} does not have expected ELF machine ${readelf_machine}"
  fi

  local size="$(stat -f%z "${dst_archive}" 2>/dev/null || stat -c%s "${dst_archive}" 2>/dev/null)"
  local sha256="$(shasum -a 256 "${dst_archive}" | awk '{print $1}')"
  local duration=$((end_epoch - start_epoch))

  manifest_entries+=("    {\"target\": \"${target}\", \"abi\": \"${arch_label}\", \"size\": ${size}, \"sha256\": \"${sha256}\", \"durationSeconds\": ${duration}}")
}

build_jni_so() {
  local target="$1"
  local abi="$2"
  local archive="$(cd "${OUTPUT_DIR}/${abi}" && pwd)/libfluxcore.a"
  local jni_build_dir="${OUTPUT_DIR}/jni-build/${abi}"
  local jni_libs_dir="${OUTPUT_DIR}/jniLibs/${abi}"

  mkdir -p "${jni_build_dir}" "${jni_libs_dir}"

  local start_epoch="$(date +%s)"
  echo "Building JNI .so for ${target} (${abi})..."

  "${CMAKE}" \
    -DCMAKE_TOOLCHAIN_FILE="${NDK_ROOT}/build/cmake/android.toolchain.cmake" \
    -DANDROID_ABI="${abi}" \
    -DANDROID_PLATFORM="android-${API_LEVEL}" \
    -DANDROID_STL=c++_static \
    -DFLUXCORE_ARCHIVE="${archive}" \
    -DFLUXCORE_HEADER_DIR="${OUTPUT_DIR}/Headers" \
    -S "${REPOSITORY_DIR}/mobile-proof/android/app/src/main/cpp" \
    -B "${jni_build_dir}" \
    >/dev/null

  "${CMAKE}" --build "${jni_build_dir}" --config "${PROFILE}" >/dev/null

  local built_so
  built_so="$(find "${jni_build_dir}" -name "libfluxcore_mobile_probe.so" -type f | head -1)"
  if [[ -z "${built_so}" ]]; then
    fail "libfluxcore_mobile_probe.so was not produced for ${target}"
  fi
  cp "${built_so}" "${jni_libs_dir}/libfluxcore_mobile_probe.so"

  local end_epoch="$(date +%s)"

  # Verify ELF machine and exported JNI symbols.
  local expected_machine
  case "${target}" in
    aarch64-linux-android) expected_machine="AArch64" ;;
    x86_64-linux-android) expected_machine="X86-64|Advanced Micro Devices X86-64" ;;
    armv7-linux-androideabi) expected_machine="ARM" ;;
  esac

  if ! "${BIN_DIR}/llvm-readelf" -h "${jni_libs_dir}/libfluxcore_mobile_probe.so" 2>/dev/null | grep -Eq "Machine:.*(${expected_machine})"; then
    fail "JNI .so for ${target} does not have expected ELF machine ${expected_machine}"
  fi

  local so_symbols
  so_symbols="$("${BIN_DIR}/llvm-readelf" --dyn-syms "${jni_libs_dir}/libfluxcore_mobile_probe.so" 2>/dev/null | grep -E 'Java_com_fluxbar_mobileproof_FluxCoreBridge_(request|initVerifierNative)' || true)"
  if [[ -z "${so_symbols}" ]]; then
    fail "JNI .so for ${target} is missing exported Java_com_fluxbar_mobileproof_FluxCoreBridge_* symbols"
  fi

  # Verify the .so only depends on expected Android system libraries. The C++
  # STL is statically linked, so libc++_shared must not appear. No OpenSSL or
  # host-specific libraries should appear.
  local needed
  needed="$("${BIN_DIR}/llvm-readelf" -d "${jni_libs_dir}/libfluxcore_mobile_probe.so" 2>/dev/null | grep 'NEEDED' || true)"
  local unexpected
  unexpected="$(printf '%s' "${needed}" | grep -Ev 'libandroid\.so|liblog\.so|libc\.so|libm\.so|libdl\.so|libstdc\+\+\.so|libgcc\.so' || true)"
  if [[ -n "${unexpected}" ]]; then
    fail "JNI .so for ${target} has unexpected DT_NEEDED entries:\n${unexpected}"
  fi

  local size="$(stat -f%z "${jni_libs_dir}/libfluxcore_mobile_probe.so" 2>/dev/null || stat -c%s "${jni_libs_dir}/libfluxcore_mobile_probe.so" 2>/dev/null)"
  local sha256="$(shasum -a 256 "${jni_libs_dir}/libfluxcore_mobile_probe.so" | awk '{print $1}')"
  local duration=$((end_epoch - start_epoch))

  so_manifest_entries+=("    {\"target\": \"${target}\", \"abi\": \"${abi}\", \"size\": ${size}, \"sha256\": \"${sha256}\", \"durationSeconds\": ${duration}}")
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

verifier_aar=""
aar_sha256=""
aar_size=""
if [[ "${BUILDING_PROOF}" -eq 1 ]]; then
  require_cmake
  verifier_aar="$(locate_verifier_aar)"
  mkdir -p "${OUTPUT_DIR}/verifier"
  cp "${verifier_aar}" "${OUTPUT_DIR}/verifier/rustls-platform-verifier-0.1.1.aar"
  aar_sha256="$(shasum -a 256 "${OUTPUT_DIR}/verifier/rustls-platform-verifier-0.1.1.aar" | awk '{print $1}')"
  aar_size="$(stat -f%z "${OUTPUT_DIR}/verifier/rustls-platform-verifier-0.1.1.aar" 2>/dev/null || stat -c%s "${OUTPUT_DIR}/verifier/rustls-platform-verifier-0.1.1.aar" 2>/dev/null)"

  build_jni_so "aarch64-linux-android" "arm64-v8a"
  build_jni_so "x86_64-linux-android" "x86_64"
  build_jni_so "armv7-linux-androideabi" "armeabi-v7a"
fi

rust_version="$(rustc --version)"
cargo_version="$(cargo --version)"
  ndk_version="$(cat "${NDK_ROOT}/source.properties" 2>/dev/null | grep 'Pkg.Revision' | awk -F= '{print $2}' | tr -d ' ' || echo 'unknown')"

manifest_entries_joined="$(IFS=,; echo "${manifest_entries[*]}")"

extra_manifest_fields=""
if [[ "${BUILDING_PROOF}" -eq 1 ]]; then
  so_manifest_entries_joined="$(IFS=,; echo "${so_manifest_entries[*]}")"
  extra_manifest_fields=",
  \"mobileRuntimeProofFeature\": true,
  \"rustlsPlatformVerifierAar\": {
    \"path\": \"verifier/rustls-platform-verifier-0.1.1.aar\",
    \"version\": \"0.1.1\",
    \"size\": ${aar_size},
    \"sha256\": \"${aar_sha256}\"
  },
  \"jniLibraries\": [
${so_manifest_entries_joined}
  ]"
fi

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
  "features": "${CARGO_FEATURES:-}",
  "buildStarted": "${build_start_global}",
  "buildFinished": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "targets": [
${manifest_entries_joined}
  ]${extra_manifest_fields}
}
EOF

echo "Android Rust core archives: ${OUTPUT_DIR}"
if [[ "${BUILDING_PROOF}" -eq 1 ]]; then
  echo "Android JNI libraries: ${OUTPUT_DIR}/jniLibs"
  echo "Android verifier AAR: ${OUTPUT_DIR}/verifier/rustls-platform-verifier-0.1.1.aar"
fi
