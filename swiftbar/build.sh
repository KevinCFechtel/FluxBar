#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONFIG_FILE="${REPOSITORY_DIR}/miniflux.env"

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
  -X "main.MINIFLUX_SERVER=${MINIFLUX_SERVER}"
  -X "main.MINIFLUX_APIKEY=${MINIFLUX_APIKEY}"
)

cd "${REPOSITORY_DIR}"
CGO_ENABLED=1 GOOS=darwin GOARCH="${GOARCH:-$(go env GOARCH)}" \
  go build -buildvcs=false -o "${SCRIPT_DIR}/fluxbar.cgo" -ldflags "${LDFLAGS[*]}" ./cmd/fluxbar-swiftbar

echo "SwiftBar-Binary erstellt: ${SCRIPT_DIR}/fluxbar.cgo"
