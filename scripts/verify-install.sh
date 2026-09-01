#!/usr/bin/env bash
# Build (if needed), run a loopback server, send text, fetch it back.
# Also sends an --agent receipt whose body is ciphertext without the key.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="${COPYPASTE_BIN:-$ROOT/target/debug/copypaste}"
if [[ ! -x "$BIN" ]]; then
  cargo build --bin copypaste
fi

PORT="${COPYPASTE_VERIFY_PORT:-18080}"
export COPYPASTE_FORCE_MEMORY=true
export COPYPASTE_REQUIRE_WRITE_AUTH=false
export ROCKET_ADDRESS=127.0.0.1
export ROCKET_PORT="$PORT"

"$BIN" serve >/tmp/copypaste-verify.log 2>&1 &
PID=$!
cleanup() { kill "$PID" 2>/dev/null || true; }
trap cleanup EXIT

ok=0
for _ in $(seq 1 50); do
  if curl -fsS "http://127.0.0.1:${PORT}/api/health" >/dev/null 2>&1; then
    ok=1
    break
  fi
  sleep 0.2
done
if [[ "$ok" != 1 ]]; then
  echo "server did not become healthy" >&2
  cat /tmp/copypaste-verify.log >&2 || true
  exit 1
fi

HOST="http://127.0.0.1:${PORT}"
DISCOVERY="$(curl -fsS "${HOST}/.well-known/copypaste.json")"
echo "$DISCOVERY" | grep -q '"copypaste":1'

PLAIN="$("$BIN" send "verify-install $(date -u +%s)" --host "$HOST")"
ID="${PLAIN##*/}"
BODY="$(curl -fsS "${HOST}/api/pastes/${ID}")"
echo "$BODY" | grep -q verify-install

RECEIPT="$("$BIN" send --agent "only another agent should read this" --host "$HOST")"
echo "$RECEIPT" | grep -q '"copypaste":1'
KEY="$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['key'])" "$RECEIPT")"
AID="$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['id'])" "$RECEIPT")"
NOKEY_CODE="$(curl -sS -o /tmp/copypaste-no-key.txt -w "%{http_code}" "${HOST}/api/pastes/${AID}")"
if [[ "$NOKEY_CODE" != "401" ]]; then
  echo "expected 401 without key, got ${NOKEY_CODE}" >&2
  exit 1
fi
SECRET="$(curl -fsS -H "X-Paste-Key: ${KEY}" "${HOST}/api/pastes/${AID}")"
echo "$SECRET" | grep -q "only another agent"

echo "verify-install: send, fetch, and agent ciphertext all ok"
