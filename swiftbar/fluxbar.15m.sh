#!/usr/bin/env bash

# <xbar.title>FluxBar</xbar.title>
# <xbar.version>v2.0</xbar.version>
# <xbar.author>KevinCFechtel</xbar.author>
# <xbar.author.github>KevinCFechtel</xbar.author.github>
# <xbar.desc>Zeigt ungelesene Miniflux-Artikel mit Feed-Icons an</xbar.desc>
# <xbar.dependencies>bash</xbar.dependencies>

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
BINARY="${FLUXBAR_BINARY:-${SCRIPT_DIR}/fluxbar.cgo}"

if [[ ! -x "${BINARY}" ]]; then
  echo "! | color=red"
  echo "---"
  echo "FluxBar-Binary fehlt: ${BINARY}"
  exit 0
fi

exec "${BINARY}" "$0" "${1:-}"
