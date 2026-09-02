#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
out="$(COPYPASTE_INSTALL_DRY_RUN=1 sh "$ROOT/scripts/install.sh" --dry-run)"
echo "$out" | grep -q "copypaste installer"
echo "$out" | grep -qE "plan: (brew|cargo|download)" || echo "$out" | grep -q "No GitHub release"
cmp -s "$ROOT/scripts/install.sh" "$ROOT/frontend/public/install.sh"
cmp -s "$ROOT/scripts/install.ps1" "$ROOT/frontend/public/install.ps1"
cmp -s "$ROOT/vercel.json" "$ROOT/frontend/vercel.json"
echo "install script ok"
