#!/bin/bash
# Start the controller web app (axum backend + vite frontend) on fixed ports.
# Usage: ./dev.sh [port]   (default: vite=1420, axum=3001)
set -euo pipefail
PORT=${1:-1420}
AXUM_PORT=${AXUM_PORT:-3001}

cleanup() {
  [[ -n "${AXUM_PID:-}" ]] && kill "$AXUM_PID" 2>/dev/null || true
  [[ -n "${VITE_PID:-}" ]] && kill "$VITE_PID" 2>/dev/null || true
}
trap cleanup EXIT

(cd server && PORT="$AXUM_PORT" cargo run --bin the-controller-server) &
AXUM_PID=$!

DEV_PORT="$PORT" AXUM_PORT="$AXUM_PORT" pnpm dev -- --strictPort --port "$PORT" &
VITE_PID=$!

wait
