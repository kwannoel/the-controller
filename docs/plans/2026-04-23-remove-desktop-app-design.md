# Remove Desktop App — Design

**Goal:** Delete the Tauri desktop target. The axum-backed web frontend (commit 4a699ae, #543) is the sole deliverable. The `src-tauri/` directory goes away entirely, along with every `@tauri-apps/*` dep and every `isTauri` branch in `src/`.

**Motivation:** Web frontend is Playwright-testable end-to-end; the Tauri target isn't. Parity is achieved for 57/59 commands — the two remaining (`capture_app_screenshot`, `copy_image_file_to_clipboard`) are reimplemented with browser APIs in this plan.

## Definition

Rip out the desktop shell; keep one frontend (`src/`) and one backend (the current `src-tauri/src/bin/server.rs` binary, relocated out of `src-tauri/`). Rewire the two native-only features so the web frontend can capture screenshots and push images onto the clipboard without Tauri.

## Constraints

- `src-tauri/src/bin/server.rs` and most of `src-tauri/src/*.rs` (auto_worker, maintainer, pty_manager, emitter, state, storage, worktree, etc.) are **shared library code**, not Tauri code. They must survive the deletion. Only the Tauri-specific layers go: `lib.rs`, `tauri.conf.json`, `build.rs`, `capabilities/`, `icons/`, `commands.rs` (the `#[tauri::command]` wrappers — the underlying functions in `commands/*.rs` stay), `main.rs` (Tauri entry), and all `tauri*` crate deps.
- Crate must be renamed from `the-controller` (the Tauri binary) to a server-only crate. New layout: move the whole `src-tauri/` contents up to a top-level `server/` — or simpler, **delete the `src-tauri/` wrapper and promote its contents**. Final layout: `server/Cargo.toml`, `server/src/lib.rs` (the current `the_controller_lib` root, minus Tauri), `server/src/bin/server.rs` → becomes `server/src/main.rs` so `cargo run` from `server/` just works.
- Browser reimplementations:
  - **Screenshot** — `html2canvas` captures the DOM. Returns a PNG dataURL; the feedback widget consumes a path today but can take a Blob/dataURL just as easily. No OS-level window capture, no cropping mode — the `cropped: true` path is dropped (document in plan, confirm with user if needed). Alternative `getDisplayMedia` requires a user gesture + permission prompt every time; skip it.
  - **Clipboard image** — `navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])`. The terminal drag-drop handler already has the file path; `fetch(fileUrl)` → `blob()` → write. Needs HTTPS or localhost (already the case for dev).
- Tests: the `@tauri-apps/*` mocks in four test files (`App.test.ts`, `AgentDashboard.test.ts`, `clipboard.test.ts`, `backend.test.ts`) go away; those tests switch to mocking `fetch` / `navigator.clipboard` / `window.open` directly.
- Playwright harness already boots axum + vite; no changes needed except dropping the chat-daemon gap tracked in the parity plan (out of scope here — keep working the same way it does now).
- `list_archived_projects` is in HTTP but not Tauri — already an HTTP-only route, no action needed on deletion.
- No backwards compat shims. No feature flags. Straight cut-over: one PR, one commit series, one merge.

## Migration plan

### Pass 1 — browser reimplementations (land first, verify in web mode)

1. Add `html2canvas` to `package.json` (runtime dep, not dev).
2. Add `src/lib/native.ts` exporting two browser-only helpers:
   - `captureScreenshot(cropped: boolean): Promise<Blob>` — calls `html2canvas(document.body)`, returns the canvas as a PNG blob. If `cropped` is true, ignore it (log a one-time warn and fall through to full capture) — cropping is dropped.
   - `copyImageBlobToClipboard(blob: Blob): Promise<void>` — writes via `navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })])`.
3. Rewrite `src/App.svelte:231` (feedback widget) to use `captureScreenshot(cropped)` directly and feed a Blob/dataURL to the feedback submission flow instead of a filesystem path. Find downstream consumers of the path and switch them to Blob.
4. Rewrite `src/lib/Terminal.svelte:252` (drag-drop image paste) to `fetch` the dropped file URL (or read the `DataTransferItem` as a Blob), then call `copyImageBlobToClipboard`.
5. Update the four affected test files to mock browser APIs instead of Tauri plugins.

**Exit:** `pnpm test` green. Manual: in a browser, drag an image onto a terminal, confirm it lands on the system clipboard; click the feedback widget screenshot button, confirm a PNG reaches the submission handler.

### Pass 2 — strip `@tauri-apps/*` from the frontend

1. `src/lib/backend.ts` — delete the Tauri branches. `command()` always `fetch`, `listen()` always WebSocket, `openUrl()` always `window.open(url, "_blank", "noopener")`. Remove the `isTauri` helper entirely (grep first to confirm no other call sites).
2. `src/App.svelte:292-302` (`updateWindowTitle`) — this sets the window title to `"The Controller (commit, branch, localhost:port)"`. The web branch already does the right thing via `document.title = title`. Keep the function and the `document.title` assignment; delete only the `if (isTauri)` branch (and the `@tauri-apps/api/window` dynamic import). No other window-control UI in the file — confirmed by `grep -n "getCurrentWindow\|@tauri-apps/api/window" src/App.svelte`.
3. `src/lib/clipboard.ts` — delete the Tauri branch; keep only the `navigator.clipboard.read()` path.
4. `package.json` — remove `@tauri-apps/api`, `@tauri-apps/plugin-clipboard-manager`, `@tauri-apps/plugin-opener`, `@tauri-apps/cli`. Remove `tauri` script.
5. `pnpm install` to refresh the lockfile.

**Exit:** `grep -r "@tauri-apps" src/ package.json` returns nothing. `pnpm build` green. `pnpm test` green.

### Pass 3 — delete the Rust Tauri layer

1. Delete `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, `src-tauri/src/commands.rs` (the Tauri wrappers — the delegate functions in `src-tauri/src/commands/*.rs` stay). Verify `server.rs` doesn't reach into `commands.rs`; it calls the domain functions directly.
2. Delete `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/`, `src-tauri/icons/`.
3. Delete the `media.rs` module entirely (screenshot + clipboard image were its only callers, both now browser-side). If any other file pulls `tauri::AppHandle`, sweep and delete — the remaining modules (`auto_worker`, `maintainer`, etc.) should already be Tauri-free or only lightly coupled via the emitter abstraction.
4. `Cargo.toml`:
   - Remove `tauri`, `tauri-build`, `tauri-plugin-opener`, `tauri-plugin-clipboard-manager` deps.
   - Remove the `[build-dependencies] tauri-build` entry.
   - Remove `server` feature gate — axum becomes unconditional.
   - Rename the crate from `the-controller` to `the-controller-server`. Rename `the_controller_lib` usage in `server.rs` to match.
   - Promote `src/bin/server.rs` to `src/main.rs`; delete the `[[bin]]` section.
5. Move `src-tauri/` contents up: `git mv src-tauri/Cargo.toml server/Cargo.toml`, same for `src/`, `tests/`, `test-data/`. Delete the now-empty `src-tauri/`.
6. Update `Cargo.lock` via `cargo build` in `server/`.
7. `dev.sh`, `scripts/*`, `playwright.config.ts`, `CLAUDE.md`, `README.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`, `agents.md`, any `docs/domain-knowledge.md` references to Tauri — sweep and update. `pnpm tauri dev` is gone; `dev.sh` already boots vite + axum for the web path, that becomes the only dev command.
8. Remove the `tauri` script from `package.json` (already gone in Pass 2) and any `@tauri-apps/cli` references in `.gitignore` / build scripts.

**Exit:** `grep -r "tauri" --include="*.rs" --include="*.toml" --include="*.ts" --include="*.svelte" --include="*.json" --include="*.md"` returns only historical references in `docs/plans/*` (fine — plans are historical). `cargo build` in `server/` green. `cargo test` green. `pnpm test` green. `pnpm build` green. `dev.sh` starts app, browser at `localhost:1420` works end-to-end.

### Pass 4 — docs + CI

1. Update `CLAUDE.md`: replace "Tauri v2 + Svelte 5 desktop app" framing with "axum + Svelte 5 web app". Remove `pnpm tauri dev` under Dev Commands. Update the domain-knowledge reference to note that the "Tauri main thread blocking" lesson is historical context for why `spawn_blocking` is used throughout, not an active constraint.
2. Update `ARCHITECTURE.md`, `README.md`, `CONTRIBUTING.md`.
3. CI: any job running `cargo tauri build` / invoking `@tauri-apps/cli` goes away. Verify GitHub Actions workflows in `.github/` (check during implementation).
4. Pre-commit hook: confirm it doesn't call tauri tooling.

## Validation

Per `CLAUDE.md` task structure, each pass has its own verification before moving on:

- **Pass 1:** `pnpm test` green after the two reimplementations; manual browser check for feedback screenshot + terminal image paste.
- **Pass 2:** `grep -r "@tauri-apps" src/ package.json` empty; `pnpm build && pnpm test` green.
- **Pass 3:** `cargo build && cargo test` from `server/` green; `grep -rE "tauri|@tauri" --include="*.rs" --include="*.ts" --include="*.svelte" --include="*.toml" --include="*.json" src/ server/ package.json Cargo.toml` empty (docs excluded); `dev.sh` boots cleanly; Playwright smoke `pnpm playwright test` green.
- **Pass 4:** no stale `pnpm tauri dev` / `cargo tauri` references in checked-in docs or CI config.

Revert-test the semantic changes: if Pass 1's browser reimplementations are reverted, the feedback screenshot test and the terminal clipboard test fail. If they apply cleanly, they pass.

## Decisions (resolved 2026-04-24)

1. **Cropped screenshots** — dropped. `html2canvas(document.body)` only; the `cropped` parameter becomes unused, callers stop passing it, the Tauri `screencapture -i` behavior is gone. Tests updated accordingly.
2. **macOS app bundle distribution** — dropped intentionally. `.dmg` / `.app` goes away; users run `dev.sh` or open the server URL.
3. **Window title** (`src/App.svelte:292-302`) — logic is already ported. Web path calls `document.title = title`; Tauri path calls `getCurrentWindow().setTitle(title)` with the same string. Cleanup keeps `document.title`, drops the `isTauri` branch and the `@tauri-apps/api/window` import. No other window APIs used in `App.svelte`.
