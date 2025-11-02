#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

echo "[1/9] Ensuring coverage directory exists"
mkdir -p coverage

echo "[2/9] Installing frontend dependencies"
if [[ -f "frontend/package-lock.json" ]]; then
  npm_install_cmd=(npm ci)
else
  npm_install_cmd=(npm install)
fi
(
  cd frontend
  "${npm_install_cmd[@]}"
)

if [[ -d "blockchain" ]]; then
  echo "[3/9] Installing blockchain dependencies"
  (
    cd blockchain
    if [[ -f package-lock.json ]]; then
      npm ci
    else
      npm install
    fi
  )
else
  echo "[3/9] Skipping blockchain dependencies (directory missing)"
fi

echo "[4/9] Running cargo fmt --check"
cargo fmt --all -- --check

echo "[5/9] Running cargo clippy"
cargo clippy --all-targets --all-features -- -D warnings

echo "[6/9] Building release binaries"
cargo build --release --all-targets

echo "[7/9] Running cargo nextest"
cargo nextest run --workspace --all-features

echo "[8/9] Generating coverage report"
cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75 --lcov --output-path coverage/lcov.info

echo "[9/9] Running frontend lint/test/build"
(
  cd frontend
  npm run lint
  npm test -- --run
  npm run build
)

echo "CI workflow completed locally. Coverage: coverage/lcov.info"

popd >/dev/null
