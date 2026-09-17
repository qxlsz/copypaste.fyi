#!/usr/bin/env bash
# Build a .deb from an already-compiled copypaste binary.
# Usage: scripts/build-deb.sh <binary> <version> [outdir]
set -euo pipefail

BIN="${1:?path to copypaste binary}"
VERSION="${2:?version (no v)}"
OUT="${3:-dist}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

if [[ ! -f "$BIN" ]]; then
  echo "missing binary $BIN" >&2
  exit 1
fi

mkdir -p "$STAGE/DEBIAN" "$STAGE/usr/bin" "$STAGE/lib/systemd/system" "$OUT"
install -m 0755 "$BIN" "$STAGE/usr/bin/copypaste"
if [[ -f "$ROOT/contrib/systemd/copypaste.service" ]]; then
  install -m 0644 "$ROOT/contrib/systemd/copypaste.service" "$STAGE/lib/systemd/system/copypaste.service"
fi

SIZE_KB="$(du -k "$STAGE/usr/bin/copypaste" | awk '{print $1}')"
cat >"$STAGE/DEBIAN/control" <<EOF
Package: copypaste
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${SIZE_KB}
Maintainer: qxlsz <qxlsz@users.noreply.github.com>
Homepage: https://www.copypaste.fyi
Description: Pastebin CLI and self-hostable server
 Type text, get a link, share /p/{id}. Same binary as copypaste.fyi.
EOF

echo "staged $STAGE"
if command -v dpkg-deb >/dev/null 2>&1; then
  DEB="$OUT/copypaste_${VERSION}_${ARCH}.deb"
  dpkg-deb --build "$STAGE" "$DEB"
  echo "wrote $DEB"
else
  cp "$STAGE/DEBIAN/control" "$OUT/control"
  echo "dpkg-deb missing; wrote $OUT/control"
fi
