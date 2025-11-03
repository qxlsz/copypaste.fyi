#!/usr/bin/env bash
set -euo pipefail

if command -v fly >/dev/null 2>&1; then
  echo "flyctl already installed: $(fly version)"
  exit 0
fi

PLATFORM=$(uname -s)
ARCH=$(uname -m)

case "${PLATFORM}" in
  Linux)
    ARCHIVE="https://fly.io/install.sh"
    curl -fsSL "$ARCHIVE" | sh
    ;;
  Darwin)
    ARCHIVE="https://fly.io/install.sh"
    curl -fsSL "$ARCHIVE" | sh
    ;;
  *)
    echo "Unsupported platform: ${PLATFORM}" >&2
    exit 1
    ;;
 esac

if ! command -v fly >/dev/null 2>&1; then
  echo "flyctl installation failed" >&2
  exit 1
fi

fly version
