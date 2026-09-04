#!/usr/bin/env bash
# Idempotent Cloud Agent bootstrap for copypaste.fyi.
# Prepares the Rust backend and the React/Vite frontend so the CI jobs in
# AGENTS.md (Rust 1.88 fmt/clippy/nextest/coverage, Node 22 frontend) run
# without further setup. The OCaml verifier is optional and not required for
# the core backend + frontend development flow.
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." &>/dev/null && pwd)"
cd "$ROOT"

# CI pins Rust 1.88.0 (see AGENTS.md). rustup ships in the base image.
RUST_VERSION=1.88.0
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$CARGO_BIN"

echo "[install] Ensuring Rust ${RUST_VERSION} toolchain"
rustup toolchain install "$RUST_VERSION" --profile minimal --no-self-update
rustup component add rustfmt clippy llvm-tools-preview --toolchain "$RUST_VERSION"
rustup default "$RUST_VERSION"

echo "[install] Ensuring cargo helper tools (nextest, llvm-cov, audit)"
if ! command -v cargo-nextest &>/dev/null; then
  curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C "$CARGO_BIN"
fi
if ! command -v cargo-llvm-cov &>/dev/null; then
  curl -LsSf https://github.com/taiki-e/cargo-llvm-cov/releases/latest/download/cargo-llvm-cov-x86_64-unknown-linux-gnu.tar.gz \
    | tar xzf - -C "$CARGO_BIN"
fi
if ! command -v cargo-audit &>/dev/null; then
  cargo install cargo-audit --locked
fi

echo "[install] Fetching Rust dependencies and warming the build cache"
cargo fetch --locked
cargo build --workspace --all-targets --all-features

echo "[install] Installing frontend dependencies"
# CI=true matches the frontend CI job and skips the Playwright browser download
# in the package "prepare" hook, keeping install deterministic and free of a
# sudo/apt dependency. Run `npx playwright install` manually for e2e browsers.
(
  cd frontend
  CI=true npm ci
)

echo "[install] Done."
