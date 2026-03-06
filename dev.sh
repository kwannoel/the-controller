#!/bin/bash
# Start the controller dev server on a custom port.
# Usage: ./dev.sh [port]   (default: 1420)
PORT=${1:-1420}
DEV_PORT=$PORT npm run tauri dev -- --config "{\"build\":{\"devUrl\":\"http://localhost:$PORT\"}}"
