#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

for name in copypaste-darwin-arm64 copypaste-darwin-x64 copypaste-linux-amd64 copypaste-linux-arm64; do
  echo "$name" >"$TMP/$name.bin"
  tar -czf "$TMP/$name.tar.gz" -C "$TMP" "$name.bin"
done

"$ROOT/scripts/bump-homebrew.sh" 0.2.0 "$TMP" "$TMP/copypaste.rb"

grep -q 'version "0.2.0"' "$TMP/copypaste.rb"
grep -q 'releases/download/v0.2.0/copypaste-darwin-arm64.tar.gz' "$TMP/copypaste.rb"
grep -q 'sha256 "' "$TMP/copypaste.rb"
grep -q 'bin.install "copypaste"' "$TMP/copypaste.rb"
echo "bump-homebrew ok"
