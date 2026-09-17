#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
grep -q 'copy_on_select' "$ROOT/contrib/terminals/ghostty.config"
grep -q 'copypaste clip' "$ROOT/contrib/terminals/install.sh"
grep -q 'iTerm2' "$ROOT/docs/terminals.md"
grep -q 'cmux' "$ROOT/docs/terminals.md"
grep -q 'Ghostty' "$ROOT/docs/terminals.md"
echo "terminals docs ok"
