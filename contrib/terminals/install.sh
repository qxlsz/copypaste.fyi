#!/usr/bin/env bash
# Install Ghostty include + print iTerm2 / cmux steps.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOST="${COPYPASTE_HOST:-http://127.0.0.1:8000}"

GHOSTTY_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/ghostty"
mkdir -p "$GHOSTTY_DIR"
cp "$ROOT/contrib/terminals/ghostty.config" "$GHOSTTY_DIR/copypaste"
if [[ -f "$GHOSTTY_DIR/config" ]] && ! grep -q 'config-file = ?copypaste' "$GHOSTTY_DIR/config"; then
  printf '\nconfig-file = ?copypaste\n' >>"$GHOSTTY_DIR/config"
elif [[ ! -f "$GHOSTTY_DIR/config" ]]; then
  printf 'config-file = ?copypaste\n' >"$GHOSTTY_DIR/config"
fi

if [[ "$(uname -s)" == "Darwin" && -x "$ROOT/contrib/macos/install-quick-action.sh" ]]; then
  "$ROOT/contrib/macos/install-quick-action.sh"
fi

cat <<EOF
Ghostty / cmux: $GHOSTTY_DIR/copypaste (copy_on_select)

Then, after a selection copy:

  copypaste clip --host $HOST

iTerm2 right-click:
  Preferences → Pointer → Right button → Run Command in Background
  copypaste clip --host $HOST

Or bind selected text directly:
  printf %s '\\(selection)' | copypaste send --stdin --host $HOST | pbcopy

Docs: docs/terminals.md
EOF
