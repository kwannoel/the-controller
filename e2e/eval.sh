#!/usr/bin/env bash
set -euo pipefail

# e2e/eval.sh — Run Playwright e2e tests against a fresh server pair from a worktree.
#
# Usage:
#   ./e2e/eval.sh <worktree-path> [test-file...]
#
# Examples:
#   ./e2e/eval.sh /path/to/worktree                         # all specs
#   ./e2e/eval.sh /path/to/worktree e2e/specs/smoke.spec.ts # one spec

WORKTREE="${1:?Usage: $0 <worktree-path> [test-file...]}"
shift
TEST_FILES=("$@")

# --- Cleanup trap ---
AXUM_PID=""
VITE_PID=""
cleanup() {
  set +e
  echo "Cleaning up..."
  [[ -n "$VITE_PID" ]] && kill "$VITE_PID" 2>/dev/null; wait "$VITE_PID" 2>/dev/null
  [[ -n "$AXUM_PID" ]] && kill "$AXUM_PID" 2>/dev/null; wait "$AXUM_PID" 2>/dev/null
}
trap cleanup EXIT

# --- Find two distinct free ports in a single call ---
read -r AXUM_PORT VITE_PORT < <(python3 -c "
import socket
def free():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(('127.0.0.1', 0))
    p = s.getsockname()[1]
    s.close()
    return p
p1, p2 = free(), free()
while p1 == p2:
    p2 = free()
print(p1, p2)
")
echo "Eval ports: Axum=$AXUM_PORT, Vite=$VITE_PORT"

# --- Ensure node_modules ---
if [[ ! -d "$WORKTREE/node_modules" ]]; then
  echo "Installing npm dependencies in worktree..."
  (cd "$WORKTREE" && npm install --silent)
fi

# --- Start Axum server ---
echo "Starting Axum server on port $AXUM_PORT..."
(cd "$WORKTREE/src-tauri" && PORT="$AXUM_PORT" cargo run --bin server --features server) &
AXUM_PID=$!

# --- Start Vite dev server ---
echo "Starting Vite dev server on port $VITE_PORT..."
(cd "$WORKTREE" && DEV_PORT="$VITE_PORT" AXUM_PORT="$AXUM_PORT" npm run dev -- --strictPort) &
VITE_PID=$!

# --- Wait for servers to be ready ---
wait_for_port() {
  local port=$1
  local label=$2
  local timeout=$3
  local pid=$4
  local elapsed=0
  echo "Waiting for $label (port $port)..."
  while ! curl -so /dev/null "http://localhost:$port" 2>/dev/null; do
    sleep 2
    elapsed=$((elapsed + 2))
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "ERROR: $label process died"
      exit 1
    fi
    if [[ $elapsed -ge $timeout ]]; then
      echo "ERROR: $label did not start within ${timeout}s"
      exit 1
    fi
  done
  echo "$label is ready on port $port"
}

wait_for_port "$VITE_PORT" "Vite" 30 "$VITE_PID"
wait_for_port "$AXUM_PORT" "Axum" 180 "$AXUM_PID"

# --- Run Playwright ---
echo "Running Playwright tests..."
PLAYWRIGHT_ARGS=(--project=e2e)
if [[ ${#TEST_FILES[@]} -gt 0 ]]; then
  PLAYWRIGHT_ARGS+=("${TEST_FILES[@]}")
fi

set +e
BASE_URL="http://localhost:$VITE_PORT" npx playwright test "${PLAYWRIGHT_ARGS[@]}"
EXIT_CODE=$?
set -e

if [[ $EXIT_CODE -eq 0 ]]; then
  echo "All tests passed."
else
  echo "Tests failed (exit code $EXIT_CODE). Check e2e/results/ for artifacts."
fi

exit $EXIT_CODE
