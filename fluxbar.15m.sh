#!/bin/bash

# <xbar.title>FluxBar</xbar.title>
# <xbar.version>v1.0</xbar.version>
# <xbar.author>KevinCFechtel</xbar.author>
# <xbar.author.github>KevinCFechtel</xbar.author.github>
# <xbar.desc>Get Article from miniflux</xbar.desc>
# <xbar.image></xbar.image>
# <xbar.dependencies>bash</xbar.dependencies>


# Start fluxbar binary
$SWIFTBAR_PLUGINS_PATH/../fluxbar/fluxbar.cgo "$0" "$1"