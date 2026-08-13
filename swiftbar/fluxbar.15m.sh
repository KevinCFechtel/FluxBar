#!/usr/bin/env bash

# <xbar.title>FluxBar</xbar.title>
# <xbar.version>v2.0</xbar.version>
# <xbar.author>KevinCFechtel</xbar.author>
# <xbar.author.github>KevinCFechtel</xbar.author.github>
# <xbar.desc>Zeigt ungelesene Miniflux-Artikel mit Feed-Icons an</xbar.desc>
# <xbar.dependencies>bash</xbar.dependencies>

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
PLUGIN_DIR="${SWIFTBAR_PLUGINS_PATH:-${SCRIPT_DIR}}"
FLUXBAR_DIR="$(cd -- "${PLUGIN_DIR}/.." && pwd)/FluxBar"

if [[ -n "${FLUXBAR_BINARY:-}" ]]; then
  BINARY="${FLUXBAR_BINARY}"
elif [[ -x "${FLUXBAR_DIR}/fluxbar.cgo" ]]; then
  BINARY="${FLUXBAR_DIR}/fluxbar.cgo"
else
  BINARY="${FLUXBAR_DIR}/swiftbar/fluxbar.cgo"
fi

if [[ ! -x "${BINARY}" ]]; then
  echo "! | color=red"
  echo "---"
  echo "FluxBar-Binary fehlt: ${BINARY}"
  exit 0
fi

exec "${BINARY}" "$0" "${1:-}"
