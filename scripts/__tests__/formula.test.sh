#!/usr/bin/env bash
# One Homebrew formula check: the bottle formula must exercise serve/send/clip/healthcheck.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FORMULA="$ROOT/Formula/copypaste.rb"
BUMP="$ROOT/scripts/bump-homebrew.sh"

grep -q 'assert_match "serve"' "$FORMULA"
grep -q 'assert_match "send"' "$FORMULA"
grep -q 'assert_match "clip"' "$FORMULA"
grep -q 'assert_match "healthcheck"' "$FORMULA"
grep -q 'brew services start copypaste' "$FORMULA"
grep -q 'assert_match "serve"' "$BUMP"
grep -q 'assert_match "clip"' "$BUMP"
grep -q 'assert_match "healthcheck"' "$BUMP"
echo "homebrew formula test ok"
