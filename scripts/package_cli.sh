#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <version>" >&2
  exit 1
fi

VERSION="$1"
SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
ARTIFACTS_DIR="$PROJECT_ROOT/dist"
BINARY_NAME="cpaste"
ARCHIVE_BASENAME="${BINARY_NAME}-${VERSION}"

pushd "$PROJECT_ROOT" >/dev/null

if ! command -v cargo &>/dev/null; then
  echo "cargo is required. Run ./scripts/install_deps.sh first." >&2
  exit 1
fi

rm -rf "$ARTIFACTS_DIR"
mkdir -p "$ARTIFACTS_DIR"

echo "Building release binary..."
cargo build --release --bin "$BINARY_NAME"

# Copy binary
BIN_PATH="target/release/${BINARY_NAME}"
if [[ ! -f "$BIN_PATH" ]]; then
  echo "Binary $BIN_PATH not found" >&2
  exit 1
fi

cp "$BIN_PATH" "$ARTIFACTS_DIR/${BINARY_NAME}"

# Create archive (tar.gz)
ARCHIVE_PATH="$ARTIFACTS_DIR/${ARCHIVE_BASENAME}.tar.gz"
pushd "$ARTIFACTS_DIR" >/dev/null
tar -czf "${ARCHIVE_BASENAME}.tar.gz" "$BINARY_NAME"
popd >/dev/null

# Generate SHA256 checksum
pushd "$ARTIFACTS_DIR" >/dev/null
shasum -a 256 "${ARCHIVE_BASENAME}.tar.gz" > "${ARCHIVE_BASENAME}.tar.gz.sha256"
popd >/dev/null

echo "Artifacts created in $ARTIFACTS_DIR:"
ls -1 "$ARTIFACTS_DIR"

cat <<EOF
Next steps:
1. Create a Git tag:   git tag -a v${VERSION} -m "Release v${VERSION}"
2. Push tag:           git push origin v${VERSION}
3. Draft release on GitHub and upload dist/${ARCHIVE_BASENAME}.tar.gz
4. Attach checksum file dist/${ARCHIVE_BASENAME}.tar.gz.sha256
EOF

popd >/dev/null
