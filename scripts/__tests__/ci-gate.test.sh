#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
grep -q "cargo audit" "$ROOT/scripts/ci-gate.sh"
grep -q "npm audit --audit-level=high" "$ROOT/scripts/ci-gate.sh"
grep -q "scripts/ci-gate.sh" "$ROOT/scripts/setup_git_hooks.sh"
grep -q "ci-gate.sh" "$ROOT/AGENTS.md"
echo "ci-gate contract ok"
