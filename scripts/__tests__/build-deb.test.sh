#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

printf '#!/bin/sh\necho copypaste\n' >"$TMP/copypaste"
chmod +x "$TMP/copypaste"
mkdir -p "$TMP/out"
bash "$ROOT/scripts/build-deb.sh" "$TMP/copypaste" 0.2.0 "$TMP/out"

if command -v dpkg-deb >/dev/null 2>&1; then
  test -f "$TMP/out/copypaste_0.2.0_$(dpkg --print-architecture).deb"
else
  grep -q 'Package: copypaste' "$TMP/out/control"
  grep -q 'Version: 0.2.0' "$TMP/out/control"
fi
echo "build-deb ok"
