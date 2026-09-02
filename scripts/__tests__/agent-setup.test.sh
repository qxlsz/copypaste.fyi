#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
chmod +x "$ROOT/scripts/agent-setup.sh"
out="$(COPYPASTE_AGENT_DRY_RUN=1 bash "$ROOT/scripts/agent-setup.sh" --dry-run --smoke)"
echo "$out" | grep -q "pkg-config"
echo "$out" | grep -q "libssl-dev"
echo "$out" | grep -q "cargo build --release"
echo "$out" | grep -q "smoke POST"
echo "agent-setup dry-run ok"
