#!/usr/bin/env bash
#
# chat-integration-daemon.sh — helper to build and/or run the
# the-controller-daemon + fake_agent pair for Task 21 chat-mode e2e tests.
#
# Usage:
#   scripts/chat-integration-daemon.sh build    # cargo build both binaries if missing
#   scripts/chat-integration-daemon.sh run      # spawn the daemon in foreground
#   scripts/chat-integration-daemon.sh status   # check whether binaries exist
#
# Environment:
#   TCD_DAEMON_REPO  Path to the the-controller-daemon checkout.
#                    Default: /Users/noelkwan/projects/the-controller-daemon
#
# When running, TCD_STATE_DIR is set to a fresh mktemp dir so the token
# lives out of the user's real $HOME/.the-controller/ and TCD_AGENT_CLAUDE_BINARY
# points at the built fake_agent. The script prints the state dir so the
# caller can locate daemon.token for the e2e harness.

set -euo pipefail

DAEMON_REPO="${TCD_DAEMON_REPO:-/Users/noelkwan/projects/the-controller-daemon}"
DAEMON_BIN="$DAEMON_REPO/target/release/the-controller-daemon"
FAKE_AGENT_BIN="$DAEMON_REPO/target/debug/fake_agent"

ensure_repo() {
  if [[ ! -d "$DAEMON_REPO" ]]; then
    echo "ERROR: daemon repo not found at $DAEMON_REPO" >&2
    echo "Set TCD_DAEMON_REPO or check out the-controller-daemon there." >&2
    exit 1
  fi
}

cmd_build() {
  ensure_repo
  if [[ ! -x "$DAEMON_BIN" ]]; then
    echo "Building the-controller-daemon (release)..."
    (cd "$DAEMON_REPO" && cargo build --release --bin the-controller-daemon)
  else
    echo "the-controller-daemon already built: $DAEMON_BIN"
  fi
  if [[ ! -x "$FAKE_AGENT_BIN" ]]; then
    echo "Building fake_agent (debug)..."
    (cd "$DAEMON_REPO" && cargo build --bin fake_agent)
  else
    echo "fake_agent already built: $FAKE_AGENT_BIN"
  fi
}

cmd_status() {
  if [[ -x "$DAEMON_BIN" ]]; then
    echo "daemon:      $DAEMON_BIN (present)"
  else
    echo "daemon:      $DAEMON_BIN (MISSING)"
  fi
  if [[ -x "$FAKE_AGENT_BIN" ]]; then
    echo "fake_agent:  $FAKE_AGENT_BIN (present)"
  else
    echo "fake_agent:  $FAKE_AGENT_BIN (MISSING)"
  fi
}

cmd_run() {
  ensure_repo
  if [[ ! -x "$DAEMON_BIN" || ! -x "$FAKE_AGENT_BIN" ]]; then
    cmd_build
  fi
  local state_dir
  state_dir="$(mktemp -d -t tcd-e2e-XXXXXX)"
  echo "TCD_STATE_DIR=$state_dir"
  echo "TCD_AGENT_CLAUDE_BINARY=$FAKE_AGENT_BIN"
  echo "Starting daemon. Ctrl-C to stop."
  TCD_STATE_DIR="$state_dir" \
    TCD_AGENT_CLAUDE_BINARY="$FAKE_AGENT_BIN" \
    exec "$DAEMON_BIN"
}

case "${1:-status}" in
  build)  cmd_build ;;
  run)    cmd_run ;;
  status) cmd_status ;;
  *)
    echo "Usage: $0 {build|run|status}" >&2
    exit 2
    ;;
esac
