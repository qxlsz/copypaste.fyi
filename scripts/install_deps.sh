#!/usr/bin/env bash
set -euo pipefail

if ! command -v rustup &>/dev/null; then
  echo "Installing Rust toolchain (rustup)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

echo "Updating Rust toolchain to stable..."
rustup toolchain install stable --profile minimal
rustup default stable

if ! command -v cargo &>/dev/null; then
  echo "cargo not found after install. Aborting." >&2
  exit 1
fi

echo "Installing cargo fmt and clippy components..."
rustup component add rustfmt clippy

echo "Installing wasm32 for rocket (optional)..."
rustup target add wasm32-unknown-unknown || true

echo "Dependencies installed successfully."
