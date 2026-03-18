#!/bin/sh
# copypaste install script
# Usage: curl -fsSL https://copypaste.fyi/install.sh | sh

set -e

REPO="qxlsz/copypaste.fyi"
INSTALL_DIR="${COPYPASTE_INSTALL_DIR:-/usr/local/bin}"
BINARY="copypaste"

# ── Detect OS and arch ────────────────────────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS_LABEL="darwin" ;;
  linux)  OS_LABEL="linux"  ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64|amd64)          ARCH_LABEL="amd64" ;;
  aarch64|arm64|armv8*)  ARCH_LABEL="arm64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

# ── Get latest version ───────────────────────────────────────────────────────
echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | sed -E 's/.*"v([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Failed to fetch latest version" >&2
  exit 1
fi

ARTIFACT="${BINARY}-${OS_LABEL}-${ARCH_LABEL}"
URL="https://github.com/$REPO/releases/download/v${LATEST}/${ARTIFACT}.tar.gz"
CHECKSUM_URL="https://github.com/$REPO/releases/download/v${LATEST}/checksums.txt"

echo "Installing copypaste v${LATEST} (${OS_LABEL}/${ARCH_LABEL})..."

# ── Download ─────────────────────────────────────────────────────────────────
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

TARBALL="$TMPDIR/$ARTIFACT.tar.gz"
curl -fsSL "$URL" -o "$TARBALL"

# ── Verify checksum (if sha256sum available) ──────────────────────────────────
if command -v sha256sum >/dev/null 2>&1; then
  EXPECTED=$(curl -fsSL "$CHECKSUM_URL" | grep "$ARTIFACT.tar.gz" | awk '{print $1}')
  if [ -n "$EXPECTED" ]; then
    ACTUAL=$(sha256sum "$TARBALL" | awk '{print $1}')
    if [ "$ACTUAL" != "$EXPECTED" ]; then
      echo "Checksum mismatch! Expected $EXPECTED, got $ACTUAL" >&2
      exit 1
    fi
  fi
fi

tar xz -C "$TMPDIR" -f "$TARBALL"

# ── Install ───────────────────────────────────────────────────────────────────
if [ -w "$INSTALL_DIR" ]; then
  install -m 755 "$TMPDIR/$BINARY" "$INSTALL_DIR/$BINARY"
else
  sudo install -m 755 "$TMPDIR/$BINARY" "$INSTALL_DIR/$BINARY"
fi

# ── Install shell completions (best effort) ────────────────────────────────────
if [ -d "/etc/bash_completion.d" ] && [ -f "$TMPDIR/completions/copypaste.bash" ]; then
  sudo cp "$TMPDIR/completions/copypaste.bash" /etc/bash_completion.d/copypaste 2>/dev/null || true
fi

if [ -d "/usr/local/share/zsh/site-functions" ] && [ -f "$TMPDIR/completions/_copypaste" ]; then
  sudo cp "$TMPDIR/completions/_copypaste" /usr/local/share/zsh/site-functions/ 2>/dev/null || true
fi

echo ""
echo "✓ copypaste ${LATEST} installed to ${INSTALL_DIR}/${BINARY}"
echo ""
echo "Quick start:"
echo "  echo 'hello world' | copypaste send"
echo "  copypaste serve          # start your own server"
echo ""
echo "Run 'copypaste --help' to get started."
