#!/bin/bash
# Start the controller dev server on a custom port.
# Usage: ./dev.sh [port]   (default: 1420)
PORT=${1:-1420}
DEV_PORT=$PORT pnpm tauri dev -- --config "{\"build\":{\"devUrl\":\"http://localhost:$PORT\"}}"
