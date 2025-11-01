#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

if ! command -v docker &>/dev/null; then
	echo "docker is required. Install Docker Desktop or the Docker CLI first." >&2
	exit 1
fi

echo "Building and starting Docker compose ..."
docker compose up --build

popd >/dev/null
