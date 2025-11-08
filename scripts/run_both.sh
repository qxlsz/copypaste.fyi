#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

if ! command -v cargo &>/dev/null; then
  echo "cargo is required. Run ./scripts/install_deps.sh first." >&2
  exit 1
fi

if ! command -v npm &>/dev/null; then
  echo "npm is required. Install Node.js LTS from https://nodejs.org/." >&2
  exit 1
fi

# Ensure backend deps are ready
cargo fetch --locked >/dev/null

# Ensure frontend deps are installed
(
  cd frontend
  if [[ -f package-lock.json ]]; then
    npm ci >/dev/null
  else
    npm install >/dev/null
  fi
)

FRONTEND_PORT=5173
FRONTEND_URL="http://127.0.0.1:${FRONTEND_PORT}"
REDIS_TCP_PORT=${REDIS_TCP_PORT:-6380}
REDIS_HTTP_PORT=${REDIS_HTTP_PORT:-8787}
REDIS_CONTAINER_NAME=${REDIS_CONTAINER_NAME:-copypaste-redis-dev}
USE_LOCAL_REDIS=${USE_LOCAL_REDIS:-true}

REDIS_CONTAINER_STARTED=false
REDIS_PROXY_PID=""

# Function to kill processes using a specific port
kill_port() {
  local port=$1
  local pids=$(lsof -ti:$port 2>/dev/null || true)
  if [[ -n "$pids" ]]; then
    echo "Killing processes using port $port: $pids"
    kill -9 $pids 2>/dev/null || true
    sleep 1
  fi
}

# Kill any existing processes using our ports
kill_port 8000  # Backend
kill_port ${REDIS_TCP_PORT}  # Redis
kill_port ${REDIS_HTTP_PORT}  # Redis proxy
kill_port ${FRONTEND_PORT}  # Frontend (try to kill 5173)
kill_port $((FRONTEND_PORT + 1))  # Frontend (try to kill 5174)
kill_port $((FRONTEND_PORT + 2))  # Frontend (try to kill 5175)
kill_port $((FRONTEND_PORT + 3))  # Frontend (try to kill 5176)

# Start Vite in the background
# Note: Frontend automatically detects development mode and uses direct backend URL
# In production, frontend uses relative /api paths (same domain as deployed app)
(
  cd frontend
  npm run dev -- --host 127.0.0.1 --port ${FRONTEND_PORT}
) &
VITE_PID=$!

cleanup() {
  echo "\nStopping frontend dev server (PID ${VITE_PID}) ..."
  kill ${VITE_PID} >/dev/null 2>&1 || true

  if [[ -n "${REDIS_PROXY_PID}" ]]; then
    echo "Stopping Redis REST proxy (PID ${REDIS_PROXY_PID}) ..."
    kill ${REDIS_PROXY_PID} >/dev/null 2>&1 || true
  fi

  if [[ "${REDIS_CONTAINER_STARTED}" == "true" ]]; then
    echo "Stopping Redis dev container (${REDIS_CONTAINER_NAME}) ..."
    docker stop "${REDIS_CONTAINER_NAME}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [[ "${USE_LOCAL_REDIS}" == "true" ]]; then
  if command -v docker &>/dev/null; then
    if docker ps -aq -f name="^${REDIS_CONTAINER_NAME}$" >/dev/null; then
      docker rm -f "${REDIS_CONTAINER_NAME}" >/dev/null 2>&1 || true
    fi

    echo "Starting Redis dev container (${REDIS_CONTAINER_NAME}) on port ${REDIS_TCP_PORT} ..."
    if docker run --rm -d --name "${REDIS_CONTAINER_NAME}" -p "${REDIS_TCP_PORT}:6379" redis:7-alpine >/dev/null 2>&1; then
      REDIS_CONTAINER_STARTED=true
    else
      echo "Warning: Failed to start Redis dev container. Falling back to in-memory persistence." >&2
    fi
  else
    echo "Docker not found. Skipping Redis dev container. Ensure Redis is running on redis://127.0.0.1:${REDIS_TCP_PORT} if you want persistence." >&2
  fi

  if [[ "${REDIS_CONTAINER_STARTED}" == "true" ]] || nc -z 127.0.0.1 "${REDIS_TCP_PORT}" >/dev/null 2>&1; then
    echo "Starting Redis REST proxy on http://127.0.0.1:${REDIS_HTTP_PORT}"
    REDIS_TCP_URL="redis://127.0.0.1:${REDIS_TCP_PORT}" REDIS_HTTP_PORT="${REDIS_HTTP_PORT}" node "${PROJECT_ROOT}/scripts/redis_proxy.js" &
    REDIS_PROXY_PID=$!
    sleep 1

    if kill -0 ${REDIS_PROXY_PID} >/dev/null 2>&1; then
      export COPYPASTE_PERSISTENCE_BACKEND=redis
      export UPSTASH_REDIS_REST_URL="http://127.0.0.1:${REDIS_HTTP_PORT}"
      export UPSTASH_REDIS_REST_TOKEN="local-dev-token"
      echo "Redis persistence enabled for local development."
    else
      echo "Warning: Redis proxy failed to start. Falling back to in-memory persistence." >&2
      REDIS_PROXY_PID=""
      if [[ "${REDIS_CONTAINER_STARTED}" == "true" ]]; then
        docker stop "${REDIS_CONTAINER_NAME}" >/dev/null 2>&1 || true
        REDIS_CONTAINER_STARTED=false
      fi
    fi
  fi
fi

echo "Frontend dev server running at ${FRONTEND_URL}"
echo "Starting Rocket backend on http://127.0.0.1:8000"
# Force memory persistence to avoid Redis connection issues
COPYPASTE_PERSISTENCE_BACKEND=memory ROCKET_ADDRESS=127.0.0.1 ROCKET_PORT=8000 cargo run --bin copypaste

popd >/dev/null
