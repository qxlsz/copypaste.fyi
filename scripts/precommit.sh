
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

usage() {
  cat <<'EOF'
Usage: scripts/precommit.sh [--skip-coverage]

Runs the same checks enforced by the git commit hooks:
  1. cargo fmt --all -- --check
  2. cargo clippy --all-targets --all-features -- -D warnings
  3. cargo build --release --all-targets
  4. cargo nextest run --workspace --all-features
  5. cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75 --lcov --output-path coverage/lcov.info
  6. npm run format (Prettier check)
  7. npm run lint (ESLint check)
  8. npm test
  9. npm run build

Pass --skip-coverage to omit the coverage step when running locally.
EOF
}

SKIP_COVERAGE=false
if [[ ${1:-} == "--help" ]]; then
  usage
  exit 0
elif [[ ${1:-} == "--skip-coverage" ]]; then
  SKIP_COVERAGE=true
elif [[ $# -gt 0 ]]; then
  usage
  exit 1
fi

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

if [[ "$SKIP_COVERAGE" == false ]]; then
  echo "[8/9] Generating coverage report"
  cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75 --lcov --output-path coverage/lcov.info
  echo "Coverage report written to coverage/lcov.info"
else
  echo "[8/9] Skipping coverage (--skip-coverage provided)"
fi

echo "[9/10] Running frontend lint/format/test/build"
(
  cd frontend
  npm run format
  npm run lint
  npm test -- --run
  npm run build
)

echo "[10/10] Validating fly.toml"
if command -v flyctl &> /dev/null; then
  flyctl config validate
else
  echo "Warning: flyctl not installed, skipping fly.toml validation"
  echo "Install with: brew install flyctl"
fi

echo "All pre-commit checks completed successfully."

popd >/dev/null
