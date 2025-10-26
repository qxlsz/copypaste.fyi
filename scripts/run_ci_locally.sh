#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

echo "[1/6] Ensuring coverage directory exists"
mkdir -p coverage

echo "[2/6] Running cargo fmt --check"
cargo fmt --all -- --check

echo "[3/6] Running cargo clippy"
cargo clippy --all-targets --all-features -- -D warnings

echo "[4/6] Building release binaries"
cargo build --release --all-targets

echo "[5/6] Running cargo nextest"
cargo nextest run --workspace --all-features

echo "[6/6] Generating coverage report"
cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75 --lcov --output-path coverage/lcov.info

echo "CI workflow completed locally. Report available at coverage/lcov.info"

popd >/dev/null
