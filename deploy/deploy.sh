#!/usr/bin/env bash
set -euo pipefail

# Deploy The Controller server mode on a Linux host.
# Uses user-level systemd — no sudo for the service itself.
#
# Usage: ./deploy.sh [--port 3001] [--host controller.example.com]
#
# Prerequisites: Rust toolchain, Node.js + pnpm, tmux, git
# Optional: Caddy (for HTTPS reverse proxy)

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INSTALL_DIR="$HOME/.the-controller"
PORT="${CONTROLLER_PORT:-3001}"
HOST=""
SKIP_BUILD=false

usage() {
  echo "Usage: $0 [OPTIONS]"
  echo ""
  echo "Options:"
  echo "  --port PORT       Server port (default: 3001)"
  echo "  --host DOMAIN     Domain for Caddy HTTPS proxy (optional)"
  echo "  --skip-build      Skip build step (use existing binaries)"
  echo "  -h, --help        Show this help"
  exit 0
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --port) PORT="$2"; shift 2 ;;
    --host) HOST="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=true; shift ;;
    -h|--help) usage ;;
    *) echo "Unknown option: $1"; usage ;;
  esac
done

echo "==> Deploying The Controller"
echo "    Install dir: $INSTALL_DIR"
echo "    Port: $PORT"
[[ -n "$HOST" ]] && echo "    Host: $HOST (Caddy HTTPS)"

# ── Check prerequisites ─────────────────────────────────────────────
check_cmd() {
  if ! command -v "$1" &>/dev/null; then
    echo "Error: $1 is required but not found."
    exit 1
  fi
}

check_cmd tmux
check_cmd git

if [[ "$SKIP_BUILD" == false ]]; then
  check_cmd rustc
  check_cmd cargo
  check_cmd node
  check_cmd pnpm
fi

# ── Build ────────────────────────────────────────────────────────────
if [[ "$SKIP_BUILD" == false ]]; then
  echo "==> Building frontend..."
  cd "$REPO_ROOT"
  pnpm install --frozen-lockfile
  pnpm build

  echo "==> Building server binary..."
  cd "$REPO_ROOT/src-tauri"
  cargo build --release --features server --bin server
fi

# ── Install ──────────────────────────────────────────────────────────
echo "==> Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR/dist"
cp "$REPO_ROOT/src-tauri/target/release/server" "$INSTALL_DIR/server"
cp -r "$REPO_ROOT/dist/." "$INSTALL_DIR/dist/"

# ── Generate auth token if not set ───────────────────────────────────
ENV_FILE="$INSTALL_DIR/server.env"
if [[ ! -f "$ENV_FILE" ]]; then
  TOKEN=$(openssl rand -hex 24)
  cat > "$ENV_FILE" <<EOF
CONTROLLER_AUTH_TOKEN=$TOKEN
CONTROLLER_PORT=$PORT
CONTROLLER_BIND=127.0.0.1
CONTROLLER_DIST_DIR=$INSTALL_DIR/dist
EOF
  chmod 600 "$ENV_FILE"
  echo "==> Generated auth token in $ENV_FILE"
else
  echo "==> Keeping existing $ENV_FILE"
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  TOKEN="${CONTROLLER_AUTH_TOKEN:-}"
fi

# ── Install user-level systemd unit ──────────────────────────────────
echo "==> Installing user systemd service..."
mkdir -p "$HOME/.config/systemd/user"
# Resolve the user's login shell and substitute it into the unit template
LOGIN_SHELL=$(getent passwd "$USER" | cut -d: -f7)
LOGIN_SHELL="${LOGIN_SHELL:-/bin/sh}"
sed "s|LOGIN_SHELL|$LOGIN_SHELL|g" \
  "$REPO_ROOT/deploy/the-controller.service" \
  > "$HOME/.config/systemd/user/the-controller.service"
systemctl --user daemon-reload
systemctl --user enable --now the-controller

# Enable lingering so the service survives logout
if command -v loginctl &>/dev/null; then
  loginctl enable-linger "$USER" 2>/dev/null || true
fi

echo "==> Service started: systemctl --user status the-controller"

# ── Caddy reverse proxy (optional) ──────────────────────────────────
if [[ -n "$HOST" ]]; then
  if ! command -v caddy &>/dev/null; then
    echo ""
    echo "Warning: --host specified but caddy is not installed. Skipping."
    echo "Install Caddy: https://caddyserver.com/docs/install"
    echo "Then copy deploy/Caddyfile to /etc/caddy/Caddyfile and edit the domain."
  else
    echo "==> Configuring Caddy for $HOST..."
    CADDY_FILE="/etc/caddy/Caddyfile"

    # Generate a site-specific Caddyfile from the template
    CADDY_SNIPPET=$(sed \
      -e "s/controller\.example\.com/$HOST/g" \
      -e "s/localhost:3001/localhost:$PORT/g" \
      "$REPO_ROOT/deploy/Caddyfile")

    if [[ -f "$CADDY_FILE" ]] && grep -q "$HOST" "$CADDY_FILE"; then
      echo "    Caddy config for $HOST already exists, skipping."
      echo "    Edit $CADDY_FILE manually if needed."
    else
      echo "$CADDY_SNIPPET" | sudo tee -a "$CADDY_FILE" > /dev/null
      sudo systemctl reload caddy
      echo "    Caddy configured: https://$HOST"
    fi
  fi
fi

# ── Print access info ────────────────────────────────────────────────
# shellcheck disable=SC1090
source "$ENV_FILE"
TOKEN="${CONTROLLER_AUTH_TOKEN:-}"

echo ""
echo "==> Deployment complete!"
if [[ -n "$HOST" ]] && command -v caddy &>/dev/null; then
  echo "    URL: https://$HOST"
else
  echo "    URL: http://localhost:$PORT"
fi
echo "    Auth token: read CONTROLLER_AUTH_TOKEN in $ENV_FILE"
echo ""
echo "    Manage:  systemctl --user {start|stop|restart|status} the-controller"
echo "    Logs:    journalctl --user -u the-controller -f"
echo "    Config:  $ENV_FILE"
