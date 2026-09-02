#!/bin/sh
# Install copypaste for this machine, then optionally host it on localhost.
#   curl -fsSL https://www.copypaste.fyi/install.sh | sh
#   curl -fsSL https://www.copypaste.fyi/install.sh | sh -s -- --serve
set -eu

REPO="qxlsz/copypaste.fyi"
INSTALL_DIR="${COPYPASTE_INSTALL_DIR:-/usr/local/bin}"
WANT_SERVE=0
DRY="${COPYPASTE_INSTALL_DRY_RUN:-0}"

for arg in "$@"; do
  case "$arg" in
    --serve) WANT_SERVE=1 ;;
    --dry-run) DRY=1 ;;
  esac
done

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
ID=""
if [ -f /etc/os-release ]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  ID=${ID:-}
fi

case "$ARCH" in
  x86_64|amd64) ARCH_LINUX="amd64"; ARCH_DARWIN="x64" ;;
  aarch64|arm64|armv8*) ARCH_LINUX="arm64"; ARCH_DARWIN="arm64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

have() { command -v "$1" >/dev/null 2>&1; }

plan() {
  echo "plan: $1"
}

install_via_brew() {
  plan "brew install qxlsz/copypaste/copypaste"
  if [ "$DRY" = "1" ]; then return 0; fi
  brew install qxlsz/copypaste/copypaste
}

install_via_cargo() {
  plan "cargo install copypaste"
  if [ "$DRY" = "1" ]; then return 0; fi
  cargo install copypaste
}

install_via_release() {
  label="$1"
  artifact="copypaste-${label}"
  latest=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/.*"tag_name": "v\([^"]*\)".*/\1/p' | head -n 1)
  if [ -z "$latest" ]; then
    return 1
  fi
  url="https://github.com/${REPO}/releases/download/v${latest}/${artifact}.tar.gz"
  plan "download ${url}"
  if [ "$DRY" = "1" ]; then return 0; fi
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  if ! curl -fsSL "$url" -o "$tmp/${artifact}.tar.gz"; then
    return 1
  fi
  tar xz -C "$tmp" -f "$tmp/${artifact}.tar.gz"
  if [ -w "$INSTALL_DIR" ]; then
    install -m 755 "$tmp/copypaste" "$INSTALL_DIR/copypaste"
  else
    sudo install -m 755 "$tmp/copypaste" "$INSTALL_DIR/copypaste"
  fi
}

hint_linux_packages() {
  case "$ID" in
    ubuntu|debian)
      echo "Ubuntu/Debian: sudo apt-get update && sudo apt-get install -y build-essential pkg-config libssl-dev curl"
      echo "Then either install Homebrew, Docker, or Rust:"
      echo "  /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
      echo "  sudo apt-get install -y docker.io docker-compose-v2 && docker compose up --build"
      echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
      ;;
    fedora)
      echo "Fedora: sudo dnf install -y gcc pkgconf openssl-devel curl"
      echo "Then either install Homebrew, Docker, or Rust:"
      echo "  /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
      echo "  sudo dnf install -y docker docker-compose && sudo systemctl enable --now docker"
      echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
      ;;
    *)
      echo "Install Homebrew, Docker, or Rust, then rerun."
      ;;
  esac
}

echo "copypaste installer  os=${OS} arch=${ARCH} distro=${ID:-unknown}"

installed=0
if have brew; then
  install_via_brew && installed=1
elif [ "$OS" = "darwin" ]; then
  if install_via_release "darwin-${ARCH_DARWIN}"; then
    installed=1
  else
    echo "No GitHub release yet. On Apple, install Homebrew then:"
    echo "  brew install qxlsz/copypaste/copypaste"
    echo "  brew services start copypaste"
  fi
elif [ "$OS" = "linux" ]; then
  if install_via_release "linux-${ARCH_LINUX}"; then
    installed=1
  elif have cargo; then
    install_via_cargo && installed=1
  else
    hint_linux_packages
  fi
elif [ "$OS" = "mingw32" ] || [ "$OS" = "msys" ] || [ "$OS" = "cygwin" ]; then
  echo "Windows: run install.ps1 in PowerShell, or Docker Desktop + docker compose up --build"
else
  echo "Unsupported OS: $OS" >&2
  exit 1
fi

if [ "$installed" = "1" ]; then
  echo "copypaste is on PATH after this shell reloads."
  echo "Send:  copypaste send --host https://www.copypaste.fyi \"notes\""
  echo "Host:  ROCKET_ADDRESS=127.0.0.1 copypaste serve"
  if have brew; then
    echo "Host:  brew services start copypaste"
  fi
  if [ "$OS" = "linux" ]; then
    echo "Host:  sudo cp contrib/systemd/copypaste.service /etc/systemd/system/ && sudo systemctl enable --now copypaste"
  fi
fi

if [ "$WANT_SERVE" = "1" ] && [ "$DRY" != "1" ]; then
  if have brew && brew services list 2>/dev/null | grep -q copypaste; then
    brew services start copypaste
  elif have copypaste; then
    ROCKET_ADDRESS=127.0.0.1 COPYPASTE_FORCE_MEMORY=true exec copypaste serve
  elif have docker; then
    exec docker compose up --build
  fi
fi
