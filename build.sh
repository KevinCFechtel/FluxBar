#!/usr/bin/env bash

source miniflux.env

LDFLAGS=(
  "-X 'main.MINIFLUX_SERVER=${MINIFLUX_SERVER}'"
  "-X 'main.MINIFLUX_APIKEY=${MINIFLUX_APIKEY}'"
)

GOOS=darwin GOARCH=arm64 go build -o fluxbar.cgo -ldflags="${LDFLAGS[*]}" fluxbar.go