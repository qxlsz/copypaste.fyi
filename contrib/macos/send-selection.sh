#!/usr/bin/env bash
# Automator Quick Action: selected text (or clipboard) → copypaste → URL on clipboard.
# Install: contrib/macos/install-quick-action.sh
set -euo pipefail

HOST="${COPYPASTE_HOST:-https://www.copypaste.fyi}"
BIN="${COPYPASTE_BIN:-copypaste}"

if ! command -v "$BIN" >/dev/null 2>&1; then
  osascript -e 'display alert "copypaste is not installed" message "brew install qxlsz/copypaste/copypaste"'
  exit 1
fi

if [[ -t 0 ]]; then
  INPUT="$("$BIN" send --clipboard --host "$HOST")"
else
  INPUT="$(cat | "$BIN" send --stdin --host "$HOST")"
fi

URL="$(printf '%s\n' "$INPUT" | awk '/https?:\/\//{print; exit}')"
if [[ -z "$URL" ]]; then
  osascript -e "display alert \"copypaste failed\" message $(printf '%q' "$INPUT")"
  exit 1
fi

printf '%s' "$URL" | pbcopy
osascript -e "display notification \"$URL\" with title \"copypaste.fyi\""
printf '%s\n' "$URL"
