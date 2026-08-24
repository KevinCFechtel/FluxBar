#!/usr/bin/env bash
set -euo pipefail

# Build the Android runtime proof host and run instrumentation tests.
#
# Usage:
#   Build/test-mobile-runtime-android.sh [artifacts-dir] [gradle-args...]
#
# Defaults:
#   artifacts-dir = .build/mobile/android
#
# Environment:
#   ANDROID_HOME           - Android SDK root (required)
#   FLUX_ANDROID_ARTIFACTS - absolute path to Rust/AAR artifacts (optional;
#                            defaults to artifacts-dir resolved against repo root)
#
# The script expects Build/build-rust-android.sh to have been run with
# CARGO_FEATURES=mobile-runtime-proof into the artifacts directory so that
# libfluxcore_mobile_probe.so and the verifier AAR are available.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

ARTIFACTS_DIR="${1:-${REPOSITORY_DIR}/.build/mobile/android}"
shift 2>/dev/null || true

if [[ ! -d "${ARTIFACTS_DIR}/jniLibs" ]]; then
  echo "Android JNI libraries not found at ${ARTIFACTS_DIR}/jniLibs" >&2
  echo "Run: CARGO_FEATURES=mobile-runtime-proof Build/build-rust-android.sh ${ARTIFACTS_DIR} release" >&2
  exit 1
fi

if [[ ! -f "${ARTIFACTS_DIR}/verifier/rustls-platform-verifier-0.1.1.aar" ]]; then
  echo "rustls-platform-verifier AAR not found at ${ARTIFACTS_DIR}/verifier/rustls-platform-verifier-0.1.1.aar" >&2
  exit 1
fi

if [[ -z "${ANDROID_HOME:-}" ]]; then
  echo "ANDROID_HOME is not set." >&2
  exit 1
fi

if ! command -v adb >/dev/null 2>&1; then
  if [[ -x "${ANDROID_HOME}/platform-tools/adb" ]]; then
    ADB="${ANDROID_HOME}/platform-tools/adb"
  else
    echo "adb not found in PATH or ANDROID_HOME/platform-tools" >&2
    exit 1
  fi
else
  ADB=adb
fi

# Gradle needs an absolute path because it resolves sourceSets relative to the
# app module directory.
FLUX_ANDROID_ARTIFACTS="$(cd "${ARTIFACTS_DIR}" && pwd)"
export FLUX_ANDROID_ARTIFACTS

ANDROID_DIR="${REPOSITORY_DIR}/mobile-proof/android"

if [[ ! -x "${ANDROID_DIR}/gradlew" ]]; then
  echo "Gradle wrapper not found at ${ANDROID_DIR}/gradlew" >&2
  echo "Generate it with: cd ${ANDROID_DIR} && gradle wrapper --gradle-version 8.13" >&2
  exit 1
fi

# Verify at least one emulator or device is connected.
connected_devices="$(${ADB} devices -l 2>/dev/null | grep -v '^List' | grep -v '^$' || true)"
if [[ -z "${connected_devices}" ]]; then
  echo "No Android emulator or device is connected." >&2
  exit 1
fi

echo "Connected devices:"
echo "${connected_devices}"

cd "${ANDROID_DIR}"

# Build and run instrumentation tests on the connected device/emulator.
./gradlew :app:connectedDebugAndroidTest --no-daemon "$@"

echo "Android runtime proof instrumentation tests passed."
