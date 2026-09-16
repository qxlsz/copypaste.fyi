#!/usr/bin/env bash
# Fast gate that matches the CI jobs that go red first (audit + fmt + workflow lint).
# Run this BEFORE git commit. pre-commit / pre-push hooks call it.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "ci-gate: workflow lint"
python3 scripts/lint-workflows.py

echo "ci-gate: cargo fmt --check"
cargo fmt --all -- --check

echo "ci-gate: cargo audit"
if ! command -v cargo-audit >/dev/null 2>&1; then
  echo "ci-gate: installing cargo-audit"
  cargo install cargo-audit --locked
fi
cargo audit

echo "ci-gate: npm audit --audit-level=high"
(
  cd frontend
  npm audit --audit-level=high
)

echo "ci-gate: ok"
