# Custom Keybindings Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Allow users to customize keyboard shortcuts via `~/.the-controller/keybindings`, with hot-reload.

**Architecture:** Rust backend parses the keybindings file and emits overrides to the frontend via Tauri events. Frontend merges overrides onto hardcoded defaults. File watcher (notify crate) detects changes and re-emits.

**Tech Stack:** Rust (notify crate for file watching, serde_json for event payloads), Svelte 5 (stores, Tauri event listener)

---

### Task 1: Rust keybindings parser — tests

**Files:**
- Create: `src-tauri/src/keybindings.rs`

**Context:** The keybindings file uses a vim-like format. Each non-comment, non-empty line is `command-name key`. Lines starting with `#` are comments. `Meta+x` means Cmd/Meta modifier. The parser returns overrides (HashMap) and warnings (Vec).

**Step 1: Write the failing tests**

Add to `src-tauri/src/keybindings.rs`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Result of parsing a keybindings file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KeybindingsResult {
    /// command_id → key overrides from the user's file
    pub overrides: HashMap<String, String>,
    /// Human-readable warnings (unknown commands, duplicate keys, parse errors)
    pub warnings: Vec<String>,
}

/// All known command IDs that can appear in the keybindings file.
const KNOWN_COMMANDS: &[&str] = &[
    "navigate-next", "navigate-prev", "expand-collapse", "fuzzy-finder",
    "create-session", "finish-branch", "save-prompt", "load-prompt", "stage",
    "screenshot", "screenshot-cropped", "toggle-session-provider",
    "new-project", "delete", "open-issues-modal", "generate-architecture",
    "toggle-help", "keystroke-visualizer",
    "toggle-agent", "trigger-agent-check", "clear-agent-reports", "toggle-maintainer-view",
    "create-note", "delete-note", "rename-note", "duplicate-note", "toggle-note-preview",
    "deploy-project", "rollback-deploy",
];

/// Parse keybindings file content into overrides + warnings.
pub fn parse_keybindings(content: &str) -> KeybindingsResult {
    todo!()
}

/// Return the path to the keybindings file.
pub fn keybindings_path(base_dir: &Path) -> PathBuf {
    base_dir.join("keybindings")
}

/// Load and parse the keybindings file. Returns empty overrides if file is missing.
pub fn load_keybindings(base_dir: &Path) -> KeybindingsResult {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_content() {
        let result = parse_keybindings("");
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_comments_only() {
        let content = "# Navigation\n# navigate-next j\n";
        let result = parse_keybindings(content);
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_single_override() {
        let content = "navigate-next h\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.get("navigate-next"), Some(&"h".to_string()));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_meta_key() {
        let content = "screenshot Meta+x\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.get("screenshot"), Some(&"Meta+x".to_string()));
    }

    #[test]
    fn test_parse_unknown_command_warns() {
        let content = "nonexistent-command x\n";
        let result = parse_keybindings(content);
        assert!(result.overrides.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("nonexistent-command"));
    }

    #[test]
    fn test_parse_malformed_line_warns() {
        let content = "just-a-word\n";
        let result = parse_keybindings(content);
        assert!(result.overrides.is_empty());
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_parse_multiple_overrides() {
        let content = "navigate-next h\nnavigate-prev l\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.len(), 2);
        assert_eq!(result.overrides.get("navigate-next"), Some(&"h".to_string()));
        assert_eq!(result.overrides.get("navigate-prev"), Some(&"l".to_string()));
    }

    #[test]
    fn test_parse_mixed_comments_and_overrides() {
        let content = "# Navigation\nnavigate-next h\n# Sessions\n# create-session c\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.len(), 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_blank_lines_ignored() {
        let content = "\n\nnavigate-next h\n\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.len(), 1);
    }

    #[test]
    fn test_parse_whitespace_trimmed() {
        let content = "  navigate-next   h  \n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.get("navigate-next"), Some(&"h".to_string()));
    }

    #[test]
    fn test_load_keybindings_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = load_keybindings(tmp.path());
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_load_keybindings_with_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("keybindings"), "navigate-next h\n").unwrap();
        let result = load_keybindings(tmp.path());
        assert_eq!(result.overrides.get("navigate-next"), Some(&"h".to_string()));
    }

    #[test]
    fn test_keybindings_path() {
        let base = Path::new("/tmp/test");
        assert_eq!(keybindings_path(base), PathBuf::from("/tmp/test/keybindings"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test keybindings -- --nocapture`
Expected: FAIL — `todo!()` panics

**Step 3: Implement the parser**

Replace the `todo!()` in `parse_keybindings`:

```rust
pub fn parse_keybindings(content: &str) -> KeybindingsResult {
    let known: std::collections::HashSet<&str> = KNOWN_COMMANDS.iter().copied().collect();
    let mut overrides = HashMap::new();
    let mut warnings = Vec::new();

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            warnings.push(format!("Line {}: malformed entry: {}", line_num + 1, line));
            continue;
        }

        let (command_id, key) = (parts[0], parts[1]);
        if !known.contains(command_id) {
            warnings.push(format!("Line {}: unknown command: {}", line_num + 1, command_id));
            continue;
        }

        overrides.insert(command_id.to_string(), key.to_string());
    }

    KeybindingsResult { overrides, warnings }
}
```

Replace the `todo!()` in `load_keybindings`:

```rust
pub fn load_keybindings(base_dir: &Path) -> KeybindingsResult {
    let path = keybindings_path(base_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_keybindings(&content),
        Err(_) => KeybindingsResult {
            overrides: HashMap::new(),
            warnings: Vec::new(),
        },
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test keybindings -- --nocapture`
Expected: All PASS

**Step 5: Commit**

```bash
git add src-tauri/src/keybindings.rs
git commit -m "feat(keybindings): add parser for keybindings config file"
```

---

### Task 2: Template generator

**Files:**
- Modify: `src-tauri/src/keybindings.rs`

**Context:** On first launch (or if file is missing), generate `~/.the-controller/keybindings` with all commands commented out, grouped by section. The template shows users what's available without activating any overrides.

**Step 1: Write the failing test**

Add to the `tests` module in `keybindings.rs`:

```rust
#[test]
fn test_generate_template_contains_all_commands() {
    let template = generate_template();
    // Every known command should appear as a comment
    for cmd in KNOWN_COMMANDS {
        assert!(template.contains(cmd), "template missing command: {}", cmd);
    }
}

#[test]
fn test_generate_template_all_lines_are_comments_or_blank() {
    let template = generate_template();
    for line in template.lines() {
        let trimmed = line.trim();
        assert!(
            trimmed.is_empty() || trimmed.starts_with('#'),
            "non-comment line in template: {}",
            line
        );
    }
}

#[test]
fn test_ensure_keybindings_file_creates_when_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    ensure_keybindings_file(tmp.path());
    let path = keybindings_path(tmp.path());
    assert!(path.exists());
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("navigate-next"));
}

#[test]
fn test_ensure_keybindings_file_does_not_overwrite_existing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = keybindings_path(tmp.path());
    std::fs::write(&path, "navigate-next h\n").unwrap();
    ensure_keybindings_file(tmp.path());
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content, "navigate-next h\n");
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test keybindings -- --nocapture`
Expected: FAIL — `generate_template` and `ensure_keybindings_file` not found

**Step 3: Implement template generator**

Add to `keybindings.rs` (above the tests module):

```rust
/// Default key for each command, used in the template.
const COMMAND_DEFAULTS: &[(&str, &str, &str)] = &[
    // (command_id, default_key, section_comment)
    // Navigation
    ("navigate-next", "j", "Navigation"),
    ("navigate-prev", "k", "Navigation"),
    ("expand-collapse", "l", "Navigation"),
    ("fuzzy-finder", "f", "Navigation"),
    // Sessions (development)
    ("create-session", "c", "Sessions (development)"),
    ("finish-branch", "m", "Sessions (development)"),
    ("save-prompt", "P", "Sessions (development)"),
    ("load-prompt", "p", "Sessions (development)"),
    ("stage", "v", "Sessions (development)"),
    ("screenshot", "Meta+s", "Sessions (development)"),
    ("screenshot-cropped", "Meta+d", "Sessions (development)"),
    ("toggle-session-provider", "Meta+t", "Sessions (development)"),
    // Projects (development)
    ("new-project", "n", "Projects (development)"),
    ("delete", "d", "Projects (development)"),
    ("open-issues-modal", "i", "Projects (development)"),
    // Panels
    ("toggle-help", "?", "Panels"),
    ("keystroke-visualizer", "Meta+k", "Panels"),
    // Agents (agents)
    ("toggle-agent", "o", "Agents (agents)"),
    ("trigger-agent-check", "r", "Agents (agents)"),
    ("clear-agent-reports", "c", "Agents (agents)"),
    ("toggle-maintainer-view", "t", "Agents (agents)"),
    // Notes (notes)
    ("create-note", "n", "Notes (notes)"),
    ("delete-note", "d", "Notes (notes)"),
    ("rename-note", "r", "Notes (notes)"),
    ("duplicate-note", "y", "Notes (notes)"),
    ("toggle-note-preview", "p", "Notes (notes)"),
    // Architecture (architecture)
    ("generate-architecture", "r", "Architecture (architecture)"),
    // Infrastructure (infrastructure)
    ("deploy-project", "d", "Infrastructure (infrastructure)"),
    ("rollback-deploy", "r", "Infrastructure (infrastructure)"),
];

pub fn generate_template() -> String {
    let mut out = String::from("# Keybindings for The Controller\n");
    out.push_str("# Uncomment and change the key to override a default binding.\n");
    out.push_str("# Format: command-name key\n");
    out.push_str("# Use Meta+ prefix for modifier keys (e.g. Meta+s)\n");
    out.push_str("# Changes are applied automatically — no restart needed.\n\n");

    let mut current_section = "";
    for &(cmd, key, section) in COMMAND_DEFAULTS {
        if section != current_section {
            if !current_section.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("# {}\n", section));
            current_section = section;
        }
        out.push_str(&format!("# {} {}\n", cmd, key));
    }

    out
}

pub fn ensure_keybindings_file(base_dir: &Path) {
    let path = keybindings_path(base_dir);
    if !path.exists() {
        let _ = std::fs::write(&path, generate_template());
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test keybindings -- --nocapture`
Expected: All PASS

**Step 5: Commit**

```bash
git add src-tauri/src/keybindings.rs
git commit -m "feat(keybindings): add template generator for default keybindings file"
```

---

### Task 3: Wire up Rust module and Tauri commands

**Files:**
- Modify: `src-tauri/src/lib.rs` (add module declaration, Tauri command registration, call ensure_keybindings_file in setup)
- Modify: `src-tauri/src/commands.rs` (add `load_keybindings` Tauri command)

**Context:** Register the keybindings module, add a Tauri command so the frontend can load keybindings on startup, and ensure the template file is created during app setup.

**Step 1: Add module to lib.rs**

In `src-tauri/src/lib.rs`, add after the other `pub mod` lines:

```rust
pub mod keybindings;
```

In the `.setup(|app| { ... })` closure, after `skills::sync_skills();`, add:

```rust
{
    let base_dir = app.state::<state::AppState>().storage.lock().unwrap().base_dir();
    keybindings::ensure_keybindings_file(&base_dir);
}
```

**Step 2: Add Tauri command**

In `src-tauri/src/commands.rs`, add:

```rust
#[tauri::command]
pub async fn load_keybindings(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<crate::keybindings::KeybindingsResult, String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    Ok(crate::keybindings::load_keybindings(&base_dir))
}
```

**Step 3: Register the command in lib.rs**

Add `commands::load_keybindings` to the `invoke_handler` list.

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands.rs src-tauri/src/keybindings.rs
git commit -m "feat(keybindings): wire up Tauri command and ensure template on startup"
```

---

### Task 4: File watcher with debounced events

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `notify` dependency)
- Modify: `src-tauri/src/keybindings.rs` (add `start_watcher` function)
- Modify: `src-tauri/src/lib.rs` (start watcher in setup)

**Context:** Use the `notify` crate to watch `~/.the-controller/keybindings` for changes. Debounce 200ms to handle rapid saves. On change, re-parse and emit `keybindings-changed` Tauri event. Keep last valid result — if parse fails mid-edit, don't emit broken state.

**Step 1: Add notify dependency**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
notify = "7"
notify-debouncer-mini = "0.5"
```

**Step 2: Implement the watcher**

Add to `keybindings.rs`:

```rust
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn start_watcher(base_dir: PathBuf, emitter: Arc<dyn crate::emitter::EventEmitter>) {
    let file_path = keybindings_path(&base_dir);
    let last_valid = Arc::new(Mutex::new(load_keybindings(&base_dir)));

    // Emit initial state
    if let Ok(payload) = serde_json::to_string(&*last_valid.lock().unwrap()) {
        let _ = emitter.emit("keybindings-changed", &payload);
    }

    let emitter_clone = emitter.clone();
    let last_valid_clone = last_valid.clone();
    let base_dir_clone = base_dir.clone();

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = match new_debouncer(Duration::from_millis(200), tx) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to create keybindings file watcher: {}", e);
                return;
            }
        };

        // Watch the base directory (the file might not exist yet)
        if let Err(e) = debouncer.watcher().watch(
            &base_dir_clone,
            notify::RecursiveMode::NonRecursive,
        ) {
            eprintln!("Failed to watch keybindings directory: {}", e);
            return;
        }

        let target = keybindings_path(&base_dir_clone);
        loop {
            match rx.recv() {
                Ok(Ok(events)) => {
                    let relevant = events.iter().any(|e| {
                        e.kind == DebouncedEventKind::Any && e.path == target
                    });
                    if !relevant {
                        continue;
                    }

                    let result = load_keybindings(&base_dir_clone);
                    // Only update last_valid if there are actual overrides or no warnings
                    // (empty file with warnings = mid-edit, keep last valid)
                    if let Ok(payload) = serde_json::to_string(&result) {
                        let _ = emitter_clone.emit("keybindings-changed", &payload);
                    }
                    *last_valid_clone.lock().unwrap() = result;
                }
                Ok(Err(e)) => {
                    eprintln!("Keybindings watcher error: {:?}", e);
                }
                Err(_) => break, // Channel closed
            }
        }
    });
}
```

**Step 3: Start watcher in lib.rs setup**

In the `.setup()` closure, after `ensure_keybindings_file`, add:

```rust
keybindings::start_watcher(base_dir.clone(), app_state.emitter.clone());
```

Note: `base_dir` is already computed from the previous step. Adjust the setup block so `base_dir` is available for both calls.

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/keybindings.rs src-tauri/src/lib.rs
git commit -m "feat(keybindings): add file watcher with debounced change events"
```

---

### Task 5: Frontend — apply overrides to commands

**Files:**
- Modify: `src/lib/commands.ts`

**Context:** Add a function that takes the default commands array and a map of overrides, returns a new commands array with keys replaced. Also update `buildKeyMap` and `getHelpSections` to accept an optional resolved commands array.

**Step 1: Write the failing test**

Create `src/lib/commands.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { applyOverrides, buildKeyMap, commands } from "./commands";

describe("applyOverrides", () => {
  it("returns defaults when no overrides", () => {
    const result = applyOverrides(commands, {});
    expect(result).toEqual(commands);
  });

  it("overrides a single key", () => {
    const result = applyOverrides(commands, { "navigate-next": "h" });
    const cmd = result.find(c => c.id === "navigate-next" && !c.hidden);
    expect(cmd?.key).toBe("h");
  });

  it("does not modify hidden aliases", () => {
    const result = applyOverrides(commands, { "expand-collapse": "x" });
    const hidden = result.filter(c => c.id === "expand-collapse" && c.hidden);
    // Hidden entries should keep their original keys
    for (const h of hidden) {
      const original = commands.find(c => c.id === h.id && c.key === h.key);
      expect(original).toBeDefined();
    }
  });

  it("overrides Meta+ commands", () => {
    const result = applyOverrides(commands, { "screenshot": "Meta+x" });
    const cmd = result.find(c => c.id === "screenshot" && !c.hidden);
    expect(cmd?.key).toBe("Meta+x");
  });

  it("ignores unknown command IDs", () => {
    const result = applyOverrides(commands, { "nonexistent": "x" });
    expect(result).toEqual(commands);
  });
});

describe("buildKeyMap with overrides", () => {
  it("uses overridden key", () => {
    const resolved = applyOverrides(commands, { "navigate-next": "h" });
    const map = buildKeyMap("development", resolved);
    expect(map.get("h")).toBe("navigate-next");
    expect(map.has("j")).toBe(false);
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `pnpm test -- --run`
Expected: FAIL — `applyOverrides` not exported

**Step 3: Implement applyOverrides**

In `src/lib/commands.ts`, add:

```typescript
export function applyOverrides(
  cmds: CommandDef[],
  overrides: Record<string, string>,
): CommandDef[] {
  if (Object.keys(overrides).length === 0) return cmds;

  return cmds.map(cmd => {
    const newKey = overrides[cmd.id];
    if (newKey && !cmd.hidden) {
      return { ...cmd, key: newKey, helpKey: undefined };
    }
    return cmd;
  });
}
```

Update `buildKeyMap` signature to accept optional resolved commands:

```typescript
export function buildKeyMap(
  mode?: WorkspaceMode,
  resolvedCommands?: CommandDef[],
): Map<string, CommandId> {
  const cmds = resolvedCommands ?? commands;
  const map = new Map<string, CommandId>();
  for (const cmd of cmds) {
    if (cmd.handledExternally) continue;
    if (mode && cmd.mode && cmd.mode !== mode) continue;
    map.set(cmd.key, cmd.id as CommandId);
  }
  return map;
}
```

**Step 4: Run tests to verify they pass**

Run: `pnpm test -- --run`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/lib/commands.ts src/lib/commands.test.ts
git commit -m "feat(keybindings): add applyOverrides and update buildKeyMap for custom bindings"
```

---

### Task 6: Frontend — keybindings store and event listener

**Files:**
- Create: `src/lib/keybindings.ts`
- Modify: `src/lib/HotkeyManager.svelte`

**Context:** Create a Svelte store that holds the current keybinding overrides. On app init, call `load_keybindings` Tauri command. Listen for `keybindings-changed` events to update the store. HotkeyManager uses the resolved commands for its keymap.

**Step 1: Create the keybindings store**

Create `src/lib/keybindings.ts`:

```typescript
import { writable, derived } from "svelte/store";
import { command, listen } from "$lib/backend";
import { commands, applyOverrides, type CommandDef } from "$lib/commands";
import { showToast } from "$lib/toast";

interface KeybindingsResult {
  overrides: Record<string, string>;
  warnings: string[];
}

export const keybindingOverrides = writable<Record<string, string>>({});

export const resolvedCommands = derived(keybindingOverrides, ($overrides) =>
  applyOverrides(commands, $overrides),
);

export async function initKeybindings() {
  try {
    const result = await command<KeybindingsResult>("load_keybindings");
    keybindingOverrides.set(result.overrides);
    for (const w of result.warnings) {
      showToast(w, "error");
    }
  } catch {
    // Silently use defaults if backend call fails
  }

  listen<string>("keybindings-changed", (raw) => {
    try {
      const result: KeybindingsResult = JSON.parse(raw);
      keybindingOverrides.set(result.overrides);
      for (const w of result.warnings) {
        showToast(w, "error");
      }
    } catch {
      // Keep current bindings on parse error
    }
  });
}
```

**Step 2: Update HotkeyManager.svelte**

In `src/lib/HotkeyManager.svelte`, replace the import and keyMap derivation:

Change:
```typescript
import { buildKeyMap, type CommandId } from "./commands";
```
To:
```typescript
import { buildKeyMap, type CommandId, type CommandDef } from "./commands";
import { resolvedCommands } from "./keybindings";
```

Change the `keyMap` derivation from:
```typescript
let keyMap = $derived(buildKeyMap(currentMode));
```
To:
```typescript
const resolvedCommandsState = fromStore(resolvedCommands);
let resolvedCmds: CommandDef[] = $derived(resolvedCommandsState.current);
let keyMap = $derived(buildKeyMap(currentMode, resolvedCmds));
```

For the externally-handled commands (Meta+key), build a lookup map from resolved commands:

```typescript
let externalKeyMap = $derived(() => {
  const map = new Map<string, string>();
  for (const cmd of resolvedCmds) {
    if (!cmd.handledExternally || cmd.hidden) continue;
    map.set(cmd.id, cmd.key);
  }
  return map;
})();
```

Then update the `onKeydown` function to use `externalKeyMap` instead of hardcoded keys. For example, replace:

```typescript
if (e.metaKey && (e.key === "s" || e.key === "d")) {
```

With a lookup that checks if the pressed Meta+key matches any external command's resolved key. The exact refactor depends on how the external commands map their keys — the key format in the commands array uses `⌘s` but the event uses `e.metaKey && e.key === "s"`. The override format uses `Meta+s`.

Helper to check external commands:

```typescript
function matchesExternalKey(cmd_id: string, e: KeyboardEvent): boolean {
  const key = externalKeyMap.get(cmd_id);
  if (!key) return false;
  if (key.startsWith("Meta+")) {
    return e.metaKey && e.key === key.slice(5);
  }
  return false;
}
```

Then refactor the Meta+key handlers to use this helper.

**Step 3: Initialize keybindings in App.svelte**

In `src/App.svelte`, import and call `initKeybindings()` in the app's initialization:

```typescript
import { initKeybindings } from "$lib/keybindings";
import { onMount } from "svelte";

onMount(() => {
  initKeybindings();
});
```

**Step 4: Verify it compiles**

Run: `pnpm check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/lib/keybindings.ts src/lib/HotkeyManager.svelte src/App.svelte
git commit -m "feat(keybindings): add store, event listener, and wire up HotkeyManager"
```

---

### Task 7: Update HotkeyHelp to show resolved keys

**Files:**
- Modify: `src/lib/commands.ts` (`getHelpSections` to accept resolved commands)
- Modify: `src/lib/HotkeyHelp.svelte` (pass resolved commands)

**Context:** The help overlay (? key) should show the user's customized keys, not the hardcoded defaults.

**Step 1: Update getHelpSections signature**

In `commands.ts`, update `getHelpSections` to accept an optional resolved commands array:

```typescript
export function getHelpSections(
  mode?: WorkspaceMode,
  resolvedCommands?: CommandDef[],
): HelpSection[] {
  const cmds = resolvedCommands ?? commands;
  // ... rest of function uses `cmds` instead of `commands`
}
```

**Step 2: Update HotkeyHelp.svelte**

Read `HotkeyHelp.svelte` first to understand its current structure, then pass `resolvedCommands` store value to `getHelpSections`.

**Step 3: Verify it compiles**

Run: `pnpm check`
Expected: No errors

**Step 4: Commit**

```bash
git add src/lib/commands.ts src/lib/HotkeyHelp.svelte
git commit -m "feat(keybindings): show resolved keys in help overlay"
```

---

### Task 8: Format, lint, and final verification

**Files:** All modified files

**Step 1: Run all checks**

```bash
pnpm check
cd src-tauri && cargo fmt --check
cd src-tauri && cargo clippy -- -D warnings
cd src-tauri && cargo test
pnpm test -- --run
```

**Step 2: Fix any issues found**

**Step 3: Final commit if needed**

```bash
git add -A
git commit -m "chore: fix format and lint issues"
```

---

## Summary of files changed

**Created:**
- `src-tauri/src/keybindings.rs` — parser, template generator, file watcher
- `src/lib/keybindings.ts` — Svelte store + event listener
- `src/lib/commands.test.ts` — frontend tests for applyOverrides

**Modified:**
- `src-tauri/Cargo.toml` — add `notify`, `notify-debouncer-mini`
- `src-tauri/src/lib.rs` — register module, setup watcher
- `src-tauri/src/commands.rs` — add `load_keybindings` command
- `src/lib/commands.ts` — add `applyOverrides`, update `buildKeyMap` and `getHelpSections`
- `src/lib/HotkeyManager.svelte` — use resolved commands, refactor external key handling
- `src/lib/HotkeyHelp.svelte` — pass resolved commands
- `src/App.svelte` — call `initKeybindings()`
