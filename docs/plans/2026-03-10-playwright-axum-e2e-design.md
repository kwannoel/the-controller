# Playwright + Axum E2E Testing & Demo Recording

## Goal

Enable Playwright-driven e2e tests and automated demo recording for the Tauri app by adding a browser-compatible backend transport (Axum HTTP + WebSocket server) alongside the existing Tauri IPC. No Electron migration needed.

## Architecture

```
Production (Tauri):
  Svelte frontend (in WKWebView) → command adapter → invoke() → Rust backend

Test/Demo (Playwright):
  Svelte frontend (in Chromium) → command adapter → HTTP/WS → Axum server → same Rust backend
```

### Command Adapter

A thin layer (`src/lib/backend.ts`) replaces all direct `invoke()` / `listen()` calls. At startup it checks `window.__TAURI__`:

- If present: uses Tauri `invoke()` and `listen()`
- If absent: uses `fetch()` for commands, WebSocket for events/PTY

```typescript
const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI__;

export async function command<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function listen<T>(event: string, handler: (payload: T) => void): () => void {
  if (isTauri) {
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<T>(event, (e) => handler(e.payload)).then((fn) => { unlisten = fn; });
    });
    return () => unlisten?.();
  }
  const ws = getSharedWebSocket();
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  ws.addEventListener("message", callback);
  return () => ws.removeEventListener("message", callback);
}
```

All existing `invoke()` and Tauri `listen()` calls get replaced with these. Mechanical find-and-replace.

### Axum Server

A new binary target: `src-tauri/src/bin/server.rs`.

```
src-tauri/
  src/
    bin/
      server.rs    ← NEW: Axum HTTP + WebSocket server
    main.rs        ← existing Tauri entry point (untouched)
    lib.rs         ← existing shared library
    commands.rs    ← existing commands (untouched)
```

The server:

- Constructs `AppState` (same as `state.rs`)
- Maps each Tauri command to `POST /api/<command_name>` → calls existing command function
- WebSocket at `/ws` for PTY output and events (replaces `app_handle.emit()`)
- PTY output broadcast: `{ event: "pty-output", payload: { sessionId, data } }`

### EventEmitter Trait

The one refactor that touches existing command code. `app_handle.emit()` calls are replaced with a `trait EventEmitter`:

- `TauriEmitter` — wraps `app_handle.emit()` (existing behavior, used in Tauri mode)
- `WsEmitter` — broadcasts to WebSocket clients (used in Axum server)

### Playwright Setup

```
e2e/
  playwright.config.ts   ← Start vite + axum server, video recording
  specs/
    smoke.spec.ts
    merge-codex.spec.ts
  helpers/
    repo-setup.ts        ← Reuse from wdio work
    project-seed.ts      ← Reuse from wdio work
  demos/                 ← Recorded videos
```

Playwright starts two servers:

1. `npm run dev` (Vite on :1420)
2. `cargo run --bin server` (Axum on :3001)

Video recording: `use: { video: "on" }` in Playwright config.

Demo script: `npm run demo` runs specs and copies `.webm` files to `demos/`.

## Migration Order

1. **Command adapter** — Create `src/lib/backend.ts`, replace all `invoke()` / `listen()` calls. Tauri app keeps working.
2. **EventEmitter trait** — Refactor `app_handle.emit()` in Rust commands to use a trait. No behavior change.
3. **Axum server binary** — Create server with routes mapping to existing commands + WebSocket for events.
4. **Playwright setup** — Config, server orchestration, smoke test.
5. **Merge-codex demo spec** — Port existing test. Real repo, real Codex, video output.
6. **Clean up wdio artifacts** — Remove wdio deps/config, replace with Playwright.

## Constraints

- Tauri desktop app must keep working identically throughout migration
- Command adapter uses runtime detection (`window.__TAURI__`), not separate builds
- Axum server reuses all existing Rust command logic — no duplication
- Only refactor needed in existing Rust code is the EventEmitter trait extraction
- Playwright runs against real backend with real PTY/git — not mocked
