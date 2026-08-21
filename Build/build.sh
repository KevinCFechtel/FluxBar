#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
APP_DIR="${REPOSITORY_DIR}/dist/FluxBar.app"
DERIVED_DATA="${REPOSITORY_DIR}/.build/DerivedData"
CONFIGURATION="${CONFIGURATION:-Debug}"
BUILT_APP="${DERIVED_DATA}/Build/Products/${CONFIGURATION}/FluxBar.app"

case "${GOARCH:-$(go env GOARCH)}" in
  arm64) BUILD_ARCH="arm64" ;;
  amd64) BUILD_ARCH="x86_64" ;;
  *)
    echo "Nicht unterstützte Architektur: ${GOARCH:-$(go env GOARCH)}" >&2
    exit 1
    ;;
esac

if [[ "${APP_DIR}" != "${REPOSITORY_DIR}/dist/FluxBar.app" ]]; then
  echo "Unerwarteter App-Pfad: ${APP_DIR}" >&2
  exit 1
fi

rm -rf -- "${APP_DIR}"
xcodebuild \
  -project "${REPOSITORY_DIR}/macos/FluxBar.xcodeproj" \
  -scheme FluxBar \
  -configuration "${CONFIGURATION}" \
  -destination "platform=macOS" \
  -derivedDataPath "${DERIVED_DATA}" \
  ARCHS="${BUILD_ARCH}" \
  ONLY_ACTIVE_ARCH=NO \
  CODE_SIGNING_ALLOWED=NO \
  build

mkdir -p "${REPOSITORY_DIR}/dist"
ditto "${BUILT_APP}" "${APP_DIR}"

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "${APP_DIR}"
fi

echo "FluxBar-App erstellt: ${APP_DIR}"
