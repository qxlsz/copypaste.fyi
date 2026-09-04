#!/usr/bin/env bash
# Rewrite Formula/copypaste.rb from GitHub release tarballs + checksums.
# Usage: scripts/bump-homebrew.sh <version> <dir-with-tarballs>
set -euo pipefail

VERSION="${1:?version (no v)}"
DIST="${2:?directory with copypaste-*.tar.gz}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FORMULA="${3:-$ROOT/Formula/copypaste.rb}"
BASE="https://github.com/qxlsz/copypaste.fyi/releases/download/v${VERSION}"

sha_of() {
  local name="$1"
  local file="$DIST/$name"
  if [[ ! -f "$file" ]]; then
    echo "missing $file" >&2
    exit 1
  fi
  sha256sum "$file" | awk '{print $1}'
}

ARM_MAC="$(sha_of copypaste-darwin-arm64.tar.gz)"
INTEL_MAC="$(sha_of copypaste-darwin-x64.tar.gz)"
AMD_LINUX="$(sha_of copypaste-linux-amd64.tar.gz)"
ARM_LINUX="$(sha_of copypaste-linux-arm64.tar.gz)"

cat >"$FORMULA" <<EOF
# Official tap: brew install qxlsz/copypaste/copypaste
# Stable URLs are GitHub Release tarballs written by scripts/bump-homebrew.sh
# on a v* tag. head: still compiles from main.
class Copypaste < Formula
  desc "Pastebin CLI and self-hostable server - type, get link, share"
  homepage "https://www.copypaste.fyi"
  license "MIT"
  version "${VERSION}"
  head "https://github.com/qxlsz/copypaste.fyi.git", branch: "main"

  on_macos do
    on_arm do
      url "${BASE}/copypaste-darwin-arm64.tar.gz"
      sha256 "${ARM_MAC}"
    end
    on_intel do
      url "${BASE}/copypaste-darwin-x64.tar.gz"
      sha256 "${INTEL_MAC}"
    end
  end

  on_linux do
    on_arm do
      url "${BASE}/copypaste-linux-arm64.tar.gz"
      sha256 "${ARM_LINUX}"
    end
    on_intel do
      url "${BASE}/copypaste-linux-amd64.tar.gz"
      sha256 "${AMD_LINUX}"
    end
  end

  depends_on "rust" => :build if build.head?

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install "copypaste"
    end
  end

  def caveats
    <<~EOS
      Public site:  copypaste send --host https://www.copypaste.fyi "notes"
      Local server: copypaste serve
                    brew services start copypaste

      Closed instance: set COPYPASTE_REQUIRE_WRITE_AUTH=true and
      COPYPASTE_AUTH_TOKEN in the environment. Tokens never go on argv.
    EOS
  end

  service do
    run [opt_bin/"copypaste", "serve"]
    keep_alive true
    working_dir var
    log_path var/"log/copypaste.log"
    error_log_path var/"log/copypaste.log"
    environment_variables ROCKET_ADDRESS: "127.0.0.1", COPYPASTE_FORCE_MEMORY: "true"
  end

  test do
    assert_match "paste", shell_output("#{bin}/copypaste --help")
  end
end
EOF

echo "wrote $FORMULA for v${VERSION}"
