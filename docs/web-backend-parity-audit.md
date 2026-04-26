# Web Backend Parity Audit

This audit tracks the removal of the Tauri desktop shell. The supported runtime
is now:

- `src/`: Svelte frontend served by Vite
- `server/`: local Axum backend exposing `/api/*` and `/ws`

The browser frontend keeps the desktop frontend's command surface by routing
former Tauri `invoke(...)` calls through HTTP. Events that used to travel
through Tauri now travel through the shared WebSocket broadcaster.

## Command Coverage

The old desktop frontend registered 59 Tauri commands. The web backend exposes
59 HTTP routes.

One desktop-native command does not have a same-name HTTP route:

- `copy_image_file_to_clipboard`: replaced by the browser drag/drop path in
  `src/lib/Terminal.svelte`, which reads dropped image files and writes them
  with `ClipboardItem`.

The backend also exposes one web-only route:

- `list_archived_projects`: supports archived project inventory reads.

## Event Coverage

The frontend listens through `src/lib/backend.ts`, which opens one shared
WebSocket connection to `/ws`. The Rust backend emits the same event names
through `server/src/emitter.rs`.

Covered event families:

- `pty-output:{session_id}`
- `session-status-changed:{session_id}`
- `session-status-hook:{session_id}`
- `session-cleanup:{session_id}`
- `staging-status`
- `merge-status`
- `secure-env-requested`
- `maintainer-status:{project_id}`
- `maintainer-error:{project_id}`
- `auto-worker-status:{project_id}`

## Regression Guard

`src/lib/web-backend-audit.test.ts` enforces three checks:

1. Every production frontend `command("...")` literal has a matching `/api/...`
   route.
2. The remaining browser replacement for desktop clipboard-image support stays
   wired.
3. Active docs do not point users at stale Tauri commands, `src-tauri/`, or
   removed workspace modes.

Run it directly with:

```bash
pnpm test src/lib/web-backend-audit.test.ts
```

Run the whole web/backend validation set with:

```bash
pnpm check
pnpm test
pnpm build
cd server && cargo fmt --check
cd server && cargo clippy -- -D warnings
cd server && cargo test
pnpm exec playwright test --project=e2e e2e/specs/smoke.spec.ts
```
