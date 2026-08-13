#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONFIG_FILE="${REPOSITORY_DIR}/miniflux.env"
APP_DIR="${REPOSITORY_DIR}/dist/FluxBar.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"

if [[ ! -f "${CONFIG_FILE}" ]]; then
  echo "Fehlende Konfiguration: ${CONFIG_FILE}" >&2
  echo "Kopiere miniflux.env.example nach miniflux.env und trage die Zugangsdaten ein." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "${CONFIG_FILE}"
: "${MINIFLUX_SERVER:?MINIFLUX_SERVER fehlt in miniflux.env}"
: "${MINIFLUX_APIKEY:?MINIFLUX_APIKEY fehlt in miniflux.env}"

LDFLAGS=(
  -s -w
  -X "main.MINIFLUX_SERVER=${MINIFLUX_SERVER}"
  -X "main.MINIFLUX_APIKEY=${MINIFLUX_APIKEY}"
)

mkdir -p "${MACOS_DIR}"
install -m 0644 "${SCRIPT_DIR}/Info.plist" "${CONTENTS_DIR}/Info.plist"

cd "${REPOSITORY_DIR}"
CGO_ENABLED=1 GOOS=darwin GOARCH="${GOARCH:-$(go env GOARCH)}" \
  go build -buildvcs=false -o "${MACOS_DIR}/FluxBar" -ldflags "${LDFLAGS[*]}" ./cmd/fluxbar-standalone

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "${APP_DIR}"
fi

echo "Standalone-App erstellt: ${APP_DIR}"
