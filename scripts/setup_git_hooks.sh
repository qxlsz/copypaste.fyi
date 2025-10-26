#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
HOOK_PATH="$PROJECT_ROOT/.git/hooks/pre-commit"

cat >"$HOOK_PATH" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo &>/dev/null; then
  echo "cargo is required for this hook" >&2
  exit 1
fi

echo "Running cargo fmt --all ..."
cargo fmt --all

if ! git diff --quiet; then
  echo "Rust formatter changed files. Re-stage and retry the commit." >&2
  exit 1
fi

echo "Running cargo clippy --all-targets --all-features ..."
cargo clippy --all-targets --all-features -- -D warnings

echo "Running cargo nextest run --workspace --all-features ..."
cargo nextest run --workspace --all-features
HOOK

chmod +x "$HOOK_PATH"

echo "Installed pre-commit hook at $HOOK_PATH"
