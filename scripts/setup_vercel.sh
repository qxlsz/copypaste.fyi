#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
FRONTEND_DIR="$PROJECT_ROOT/frontend"

if ! command -v npm &>/dev/null; then
  echo "npm is required to install the Vercel CLI. Please install Node.js LTS first." >&2
  exit 1
fi

echo "[1/3] Ensuring Vercel CLI is installed"
if ! command -v vercel &>/dev/null; then
  npm install -g vercel
else
  echo "Vercel CLI already installed ($(vercel --version | head -n 1))."
fi

echo "[2/3] Checking Vercel authentication"
if vercel whoami &>/dev/null; then
  echo "Already logged in to Vercel ($(vercel whoami))."
else
  echo "You are not logged in. Launching 'vercel login'..."
  vercel login
fi

echo "[3/3] Offering project link (optional)"
if [[ -d "$FRONTEND_DIR" ]]; then
  if [[ -f "$FRONTEND_DIR/.vercel/project.json" ]]; then
    echo "Project already linked: $(jq -r '.projectName' "$FRONTEND_DIR/.vercel/project.json")"
  else
    read -rp "Link the frontend project now? [y/N] " link_now
    if [[ ${link_now:-} =~ ^([yY][eE][sS]?|[yY])$ ]]; then
      (cd "$FRONTEND_DIR" && vercel link)
    else
      echo "Skipping 'vercel link'. You can run it later from $FRONTEND_DIR."
    fi
  fi
else
  echo "Frontend directory not found at $FRONTEND_DIR" >&2
fi

echo "Vercel CLI setup complete. To deploy, run 'vercel --cwd frontend --prod'."
