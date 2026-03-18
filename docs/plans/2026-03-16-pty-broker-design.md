# PTY Broker Design

## Overview

Replace tmux-based session persistence with a dedicated PTY broker daemon. The broker holds PTY master file descriptors in a long-lived process, communicating with the app over Unix domain sockets. This eliminates tmux as a dependency, removes its environment-variable quirks, and gives us direct control over the PTY lifecycle.

## Motivation

Tmux introduced several pain points:

- **Environment confusion**: tmux server inherits the environment of whoever started it first, not the attaching client. This caused `CLAUDECODE` and `PATH` to be stale or missing.
- **Two-layer PTY**: tmux session + local attachment PTY added latency and complexity.
- **Binary resolution**: finding the right tmux binary across Homebrew, system, and nix paths was fragile.
- **Raw byte transmission**: `send-keys -H` for non-printable bytes was a workaround, not a design.

The broker daemon solves all of these by owning PTYs directly and exposing a simple socket protocol.

## Architecture

### Components

```
┌─────────────┐         ┌──────────────┐
│  Tauri App  │◄───────►│  PTY Broker  │
│ (frontend)  │  Unix   │  (daemon)    │
│             │ sockets │              │
└─────────────┘         │  ┌────────┐  │
                        │  │ PTY fd │  │
                        │  │ PTY fd │  │
                        │  │  ...   │  │
                        │  └────────┘  │
                        └──────────────┘
```

- **`pty-broker`** — standalone Rust binary, daemonizes via double-fork. Manages PTY master fds using `portable-pty`. Auto-exits after 60s idle (no live sessions).
- **`BrokerClient`** — synchronous Rust client in the Tauri process. Auto-spawns the broker if the binary exists and it's not running. Cleans up stale PID files.
- **`broker_protocol`** — shared message types and binary frame encoding used by both sides.

### Socket Types

Two socket types per broker instance, stored in `/tmp/the-controller/`:

1. **Control socket** (`pty-broker.sock`) — request/response. Commands: `Spawn`, `Kill`, `Resize`, `List`, `HasSession`, `Shutdown`.
2. **Data sockets** (`pty-{uuid}.sock`) — raw bidirectional byte streams. One per session. The broker replays a 64KB ring buffer on connect for seamless reattachment.

Sockets live in `/tmp` rather than `~/.the-controller/` because `/tmp` is cleared on reboot. This ensures stale sockets from a previous boot are automatically cleaned up — no need for manual garbage collection on startup.

### Binary Frame Protocol

```
[u8 message_type][u32 payload_length][JSON payload]
```

Used on the control socket only. Data sockets carry raw bytes with no framing.

### Session Lifecycle

```
App start
  → BrokerClient::new() connects to control socket
  → If broker not running, auto-spawn from ~/.the-controller/bin/pty-broker
  → spawn(session_id, shell, cwd, env) → broker forks PTY, creates data socket
  → App connects to data socket, receives ring buffer replay
  → Bidirectional I/O flows over data socket

App restart
  → BrokerClient reconnects to existing broker
  → has_session(id) confirms session is alive
  → App reconnects to data socket, gets ring buffer replay
  → Session continues seamlessly

App exit (release builds)
  → BrokerClient::shutdown() tells broker to clean up all sessions and exit
```

### Graceful Degradation

`PtyManager` tries broker first, falls back to direct PTY (`Session::Pty`) if the broker binary isn't installed or the daemon can't start. This keeps dev builds working without installing the broker.

### Failure Modes

The broker can fail at three points. Each has a defined recovery path.

**1. Broker binary missing or won't start**

`try_spawn_broker()` calls `connect_control()` which attempts to spawn the binary and connect. If the binary doesn't exist or the daemon crashes on startup, `connect_control()` returns `Err` and `try_spawn_broker()` returns `false`. `spawn_session` falls through to `spawn_direct_session()`. The session works but won't survive app restarts.

**2. Broker reachable, but spawn/control request fails**

We're inside `spawn_broker_session`. `self.broker.spawn()` or `self.broker.resize()` returns `Err`. We fall back to `spawn_direct_session()` so the user still gets a working session.

**3. Broker spawn succeeds, but data socket connect fails**

The PTY exists in the broker but we can't attach to it. We kill the broker session (to avoid an orphan) and fall back to `spawn_direct_session()`.

In all three cases the invariant is: `spawn_session` either returns a working session or a hard error from the direct PTY path. The broker is never a single point of failure.

**4. App restarts with sessions not in the broker**

On startup, `restore_startup_state` checks each auto-worker session against the broker via `has_session()`. If the session isn't live in the broker (e.g., broker restarted, or session predates the broker), the session is still restored — `spawn_session` is called with `continue_session: true`, which spawns a fresh PTY with `--continue` so claude resumes where it left off. Per project, the highest-ordinal candidate is preferred; if a live broker session exists, it takes priority over non-live ones.

### Daemon Lifetime

The broker runs as a persistent daemon — no idle timeout. It stays alive until explicitly shut down via the `Shutdown` command (sent by the app on exit in release builds) or killed by a signal. This ensures sessions are always available for reattachment across app restarts.

## Key Design Decisions

1. **Idempotent spawn** — spawning the same session ID twice is a no-op, preventing duplicate processes on reconnect.
2. **Explicit environment** — full env map sent in each Spawn request with `CLAUDECODE` removed and `PATH` prepended. No inherited-environment surprises.
3. **Ring buffer replay** — 64KB circular buffer per data socket. On reconnect, the client gets recent output immediately without re-running commands.
4. **Persistent daemon** — broker stays alive until explicitly shut down or killed by signal. No idle timeout.
5. **Synchronous client** — `BrokerClient` uses blocking I/O, suitable for use behind `std::sync::Mutex` in the Tauri process. Avoids async complexity at the call sites.
6. **Resume on restart** — non-live sessions are restored with `--continue` instead of being cleaned up, so work is never lost across app or broker restarts.

## Files

### New

| File | Role |
|------|------|
| `src-tauri/src/bin/pty_broker.rs` | Broker daemon binary |
| `src-tauri/src/broker_protocol.rs` | Shared message types and frame encoding |
| `src-tauri/src/broker_client.rs` | Synchronous client, auto-spawns broker |
| `src-tauri/tests/broker_e2e.rs` | End-to-end tests against real broker |

### Modified

| File | Change |
|------|--------|
| `src-tauri/src/pty_manager.rs` | Dual-mode sessions (Broker / direct PTY), removed tmux code |
| `src-tauri/src/lib.rs` | Exit handler calls broker shutdown instead of tmux kill |
| `src-tauri/src/cli_install.rs` | Installs `pty-broker` binary alongside `controller-cli` |
| `src-tauri/src/auto_worker.rs` | Uses `BrokerClient::has_session()` instead of tmux |
| `src-tauri/Cargo.toml` | Added `[[bin]]` entry for `pty-broker` |

### Deleted

| File | Reason |
|------|--------|
| `src-tauri/src/tmux.rs` | Entire tmux integration layer replaced by broker |

## Testing

- **Protocol roundtrip tests** in `broker_protocol.rs` — all message types encode/decode correctly.
- **E2E tests** in `broker_e2e.rs` — spawn, kill, resize, data socket I/O, ring buffer replay, shutdown cleanup, multiple sessions, child exit detection. Tests spawn a real `pty-broker` binary.

## Not in v1

- Multi-client attach to the same data socket (future multi-window support)
- TLS or authentication on sockets (localhost-only, user-owned directory)
- Configurable ring buffer size

## Versioning

### Build Date

A `BUILD_DATE` environment variable is baked into all crate targets at compile time via `build.rs` (format: `YYYY-MM-DD`). Both `pty-broker` and `controller-cli` support a `--build-date` flag that prints this date and exits.

### Version-Gated Installation

`cli_install.rs` queries the installed binary's build date before copying. If the installed binary's `--build-date` output matches the app's own `BUILD_DATE`, the copy is skipped. On any failure (binary missing, no `--build-date` support, different date), the copy proceeds.

### Stale Broker Detection

`BrokerClient::connect_control()` checks the installed broker binary's build date against the app's. If they differ, the running broker is shut down and a fresh one is spawned. This handles the case where the app was updated but the old broker is still running.
