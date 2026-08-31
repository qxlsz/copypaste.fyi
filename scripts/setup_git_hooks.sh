#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"
if [[ -f "$PROJECT_ROOT/.git" ]]; then
  # worktree: .git is a file pointing at the common dir
  GITDIR=$(sed -n 's/^gitdir: //p' "$PROJECT_ROOT/.git")
  HOOKS_DIR="$GITDIR/hooks"
fi

HOOK_PATH="$HOOKS_DIR/pre-commit"
PREPUSH_PATH="$HOOKS_DIR/pre-push"

mkdir -p "$HOOKS_DIR"

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

cat >"$PREPUSH_PATH" <<'HOOK'
#!/usr/bin/env bash
# Refuse to push if the same CI jobs would fail.
set -euo pipefail

echo "pre-push: cargo fmt --check"
cargo fmt --all -- --check

echo "pre-push: cargo clippy -D warnings"
cargo clippy --all-targets --all-features -- -D warnings

echo "pre-push: cargo nextest"
cargo nextest run --workspace --all-features

if git diff --name-only origin/HEAD...HEAD 2>/dev/null | grep -q '^frontend/'; then
  echo "pre-push: frontend lint + tests"
  (cd frontend && npm test -- --run && npm run lint)
fi

echo "pre-push: ok — still wait for GitHub Actions after the push."
HOOK

chmod +x "$PREPUSH_PATH"

echo "Installed pre-commit hook at $HOOK_PATH"
echo "Installed pre-push hook at $PREPUSH_PATH"
