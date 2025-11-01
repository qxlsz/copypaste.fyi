#!/usr/bin/env bash
set -euo pipefail

kill_port() {
  local port=$1
  if command -v lsof &>/dev/null; then
    local pids
    pids=$(lsof -ti tcp:"${port}" || true)
    if [[ -n "${pids}" ]]; then
      echo "Killing processes on port ${port}: ${pids}"
      echo "${pids}" | xargs -r kill
    else
      echo "No process found on port ${port}."
    fi
  else
    echo "lsof not available; attempting pkill by port ${port}" >&2
    pkill -f ":${port}" || true
  fi
}

kill_name() {
  local name=$1
  if pkill -f "${name}" 2>/dev/null; then
    echo "Terminated processes matching '${name}'."
  else
    echo "No processes matching '${name}' were running."
  fi
}

# Stop services by port first (frontend and backend defaults)
kill_port 5173
kill_port 8000

# Fallbacks for known process names
kill_name "vite"
kill_name "copypaste"
kill_name "cargo run --bin copypaste"

echo "Done."
