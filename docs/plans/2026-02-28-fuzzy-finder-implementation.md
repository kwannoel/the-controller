# Fuzzy Finder & Streamlined UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace manual path typing with a configured projects root, fuzzy finder modal, LLM-generated project names, and an onboarding flow.

**Architecture:** Add a `Config` struct persisted to `~/.the-controller/config.json` that stores the projects root. New backend commands handle onboarding checks, directory listing, and shelling out to `claude` CLI for name generation. Three new frontend components: `Onboarding.svelte`, `FuzzyFinder.svelte`, `NewProjectModal.svelte`. App.svelte conditionally renders onboarding or main layout.

**Tech Stack:** Rust (Tauri commands, std::process::Command for CLI), Svelte 5, existing xterm.js/stores infrastructure

**Design doc:** `docs/plans/2026-02-28-fuzzy-finder-design.md`

---

### Task 1: Config Model and Onboarding Backend

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write failing tests for Config model**

Create `src-tauri/src/config.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects_root: String,
}

pub fn config_path(base_dir: &PathBuf) -> PathBuf {
    base_dir.join("config.json")
}

pub fn load_config(base_dir: &PathBuf) -> Option<Config> {
    let path = config_path(base_dir);
    if !path.exists() {
        return None;
    }
    let json = fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

pub fn save_config(base_dir: &PathBuf, config: &Config) -> std::io::Result<()> {
    let path = config_path(base_dir);
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_config_missing() {
        let tmp = TempDir::new().unwrap();
        let result = load_config(&tmp.path().to_path_buf());
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_config() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_path_buf();
        let config = Config {
            projects_root: "/tmp/projects".to_string(),
        };
        save_config(&base, &config).unwrap();
        let loaded = load_config(&base).unwrap();
        assert_eq!(loaded.projects_root, "/tmp/projects");
    }

    #[test]
    fn test_check_claude_cli_returns_valid_status() {
        // This tests that the function runs without panicking
        // Actual result depends on whether claude is installed
        let status = check_claude_cli_status();
        assert!(
            status == "authenticated"
                || status == "not_authenticated"
                || status == "not_installed"
        );
    }
}

/// Check claude CLI installation and auth status.
/// Returns "authenticated", "not_authenticated", or "not_installed".
pub fn check_claude_cli_status() -> String {
    use std::process::Command;

    // Check if claude exists
    let which = Command::new("which").arg("claude").output();
    match which {
        Ok(output) if output.status.success() => {}
        _ => return "not_installed".to_string(),
    }

    // Check auth by running a simple command
    let auth_check = Command::new("claude")
        .args(["--print", "say ok"])
        .output();
    match auth_check {
        Ok(output) if output.status.success() => "authenticated".to_string(),
        _ => "not_authenticated".to_string(),
    }
}
```

**Step 2: Run tests**

Run: `cd src-tauri && cargo test config`
Expected: 3 tests pass (load_missing, save_and_load, cli_check).

**Step 3: Add onboarding commands to commands.rs**

Add to `src-tauri/src/commands.rs`:

```rust
use crate::config;

#[tauri::command]
pub fn check_onboarding(state: State<AppState>) -> Result<Option<config::Config>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    Ok(config::load_config(&storage.base_dir()))
}

#[tauri::command]
pub fn save_onboarding_config(
    state: State<AppState>,
    projects_root: String,
) -> Result<(), String> {
    let path = std::path::Path::new(&projects_root);
    if !path.is_dir() {
        return Err(format!("Directory does not exist: {}", projects_root));
    }
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let cfg = config::Config { projects_root };
    config::save_config(&storage.base_dir(), &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_claude_cli() -> Result<String, String> {
    Ok(config::check_claude_cli_status())
}
```

**Step 4: Expose base_dir on Storage**

Add to `src-tauri/src/storage.rs`:

```rust
pub fn base_dir(&self) -> PathBuf {
    self.base_dir.clone()
}
```

**Step 5: Wire module and commands into lib.rs**

Add `pub mod config;` to lib.rs. Register `check_onboarding`, `save_onboarding_config`, `check_claude_cli` in the invoke handler.

**Step 6: Verify compilation and tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (10 existing + 3 new = 13).

Run: `cd src-tauri && cargo build`
Expected: Compiles.

**Step 7: Commit**

```bash
git add src-tauri/
git commit -m "feat: add config model and onboarding backend commands"
```

---

### Task 2: Directory Listing and Name Generation Backend

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write directory listing test**

Add to `src-tauri/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
}

pub fn list_directories(root: &str) -> std::io::Result<Vec<DirEntry>> {
    let mut entries = vec![];
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden directories
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path().to_string_lossy().to_string();
            entries.push(DirEntry { name, path });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn generate_names_via_cli(description: &str) -> Result<Vec<String>, String> {
    use std::process::Command;

    let prompt = format!(
        "Suggest 3 short, lowercase, hyphenated project directory names for: {}. Return only the 3 names, one per line, nothing else.",
        description
    );

    let output = Command::new("claude")
        .args(["--print", &prompt])
        .output()
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        return Err("claude CLI returned an error".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let names: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .take(3)
        .collect();

    if names.is_empty() {
        return Err("No names generated".to_string());
    }

    Ok(names)
}
```

**Step 2: Write test for list_directories**

Add to config.rs tests:

```rust
#[test]
fn test_list_directories() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("project-a")).unwrap();
    fs::create_dir(tmp.path().join("project-b")).unwrap();
    fs::create_dir(tmp.path().join(".hidden")).unwrap();
    fs::write(tmp.path().join("file.txt"), "not a dir").unwrap();

    let dirs = list_directories(tmp.path().to_str().unwrap()).unwrap();
    assert_eq!(dirs.len(), 2);
    assert_eq!(dirs[0].name, "project-a");
    assert_eq!(dirs[1].name, "project-b");
}
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test config`
Expected: 4 tests pass.

**Step 4: Add Tauri commands**

Add to `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn list_root_directories(state: State<AppState>) -> Result<Vec<config::DirEntry>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let cfg = config::load_config(&storage.base_dir())
        .ok_or("No config found. Complete onboarding first.")?;
    config::list_directories(&cfg.projects_root).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn generate_project_names(description: String) -> Result<Vec<String>, String> {
    config::generate_names_via_cli(&description)
}
```

**Step 5: Register commands in lib.rs**

Add `commands::list_root_directories` and `commands::generate_project_names` to invoke handler.

**Step 6: Verify**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

Run: `cd src-tauri && cargo build`
Expected: Compiles.

**Step 7: Commit**

```bash
git add src-tauri/
git commit -m "feat: add directory listing and name generation backend"
```

---

### Task 3: Scaffold Project Command and Auto-Label Sessions

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add scaffold_project command**

Add to `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn scaffold_project(state: State<AppState>, name: String) -> Result<Project, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let cfg = config::load_config(&storage.base_dir())
        .ok_or("No config found. Complete onboarding first.")?;

    let repo_path = std::path::Path::new(&cfg.projects_root).join(&name);

    // Create directory
    std::fs::create_dir_all(&repo_path).map_err(|e| e.to_string())?;

    // Git init
    git2::Repository::init(&repo_path).map_err(|e| e.to_string())?;

    // Create project entry
    let project = Project {
        id: Uuid::new_v4(),
        name,
        repo_path: repo_path.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        sessions: vec![],
    };
    storage.save_project(&project).map_err(|e| e.to_string())?;

    // Create default agents.md
    storage
        .save_agents_md(project.id, DEFAULT_AGENTS_MD)
        .map_err(|e| e.to_string())?;

    Ok(project)
}
```

**Step 2: Modify create_session to auto-generate labels**

Change the `create_session` command signature. Remove the `label` parameter and auto-generate it:

Current signature:
```rust
pub fn create_session(state, app_handle, project_id, label) -> Result<String, String>
```

New signature:
```rust
pub fn create_session(state: State<AppState>, app_handle: AppHandle, project_id: String) -> Result<String, String>
```

Inside the function, replace the `label` usage with auto-generation:

```rust
// Auto-generate label: session-N where N is next available number
let next_num = project.sessions.iter()
    .filter(|s| s.worktree_branch.is_none())
    .count() + 1;
let label = format!("session-{}", next_num);
```

**Step 3: Update Sidebar.svelte to not pass label**

In `src/lib/Sidebar.svelte`, update `createSession`:

```typescript
// Old:
const sessionId = await invoke<string>("create_session", {
  projectId,
  label: `Session ${Date.now().toString(36)}`,
});

// New:
const sessionId = await invoke<string>("create_session", {
  projectId,
});
```

**Step 4: Register scaffold_project in lib.rs**

Add `commands::scaffold_project` to invoke handler.

**Step 5: Verify**

Run: `cd src-tauri && cargo build`
Expected: Compiles.

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

Run: `npm run build`
Expected: Frontend compiles.

**Step 6: Commit**

```bash
git add src-tauri/ src/
git commit -m "feat: add scaffold_project command and auto-label sessions"
```

---

### Task 4: Onboarding Frontend Component

**Files:**
- Create: `src/lib/Onboarding.svelte`
- Create: `src/lib/onboarding.ts`
- Modify: `src/lib/stores.ts`

**Step 1: Add config store**

Add to `src/lib/stores.ts`:

```typescript
export interface Config {
  projects_root: string;
}

export const appConfig = writable<Config | null>(null);
export const onboardingComplete = writable<boolean>(false);
```

**Step 2: Create Onboarding.svelte**

Create `src/lib/Onboarding.svelte`:

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { appConfig, onboardingComplete } from "./stores";
  import { showToast } from "./toast";

  let step = $state<1 | 2>(1);
  let projectsRoot = $state("");
  let claudeStatus = $state<"checking" | "authenticated" | "not_authenticated" | "not_installed">("checking");

  async function handleNextStep1() {
    if (!projectsRoot.trim()) return;
    try {
      await invoke("save_onboarding_config", { projectsRoot: projectsRoot.trim() });
      step = 2;
      await checkClaude();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function checkClaude() {
    claudeStatus = "checking";
    try {
      const status = await invoke<string>("check_claude_cli");
      claudeStatus = status as typeof claudeStatus;
    } catch (e) {
      claudeStatus = "not_installed";
    }
  }

  function finishOnboarding() {
    appConfig.set({ projects_root: projectsRoot.trim() });
    onboardingComplete.set(true);
  }
</script>

<div class="onboarding">
  <div class="card">
    {#if step === 1}
      <h1>Welcome to The Controller</h1>
      <p>Where do your projects live?</p>
      <input
        type="text"
        bind:value={projectsRoot}
        placeholder="~/projects"
        onkeydown={(e) => e.key === "Enter" && handleNextStep1()}
      />
      <button onclick={handleNextStep1} disabled={!projectsRoot.trim()}>
        Next
      </button>
    {:else}
      <h1>Claude CLI</h1>

      {#if claudeStatus === "checking"}
        <p>Checking Claude CLI...</p>
      {:else if claudeStatus === "authenticated"}
        <p class="success">Claude CLI is ready.</p>
        <button onclick={finishOnboarding}>Get Started</button>
      {:else if claudeStatus === "not_authenticated"}
        <p class="warning">Claude CLI found but not authenticated.</p>
        <p class="hint">Run <code>claude login</code> in your terminal, then:</p>
        <button onclick={checkClaude}>Check Again</button>
      {:else}
        <p class="warning">Claude CLI not found.</p>
        <p class="hint">Install it, then:</p>
        <button onclick={checkClaude}>Check Again</button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .onboarding {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #11111b;
    color: #cdd6f4;
  }
  .card {
    background: #1e1e2e;
    padding: 40px;
    border-radius: 12px;
    border: 1px solid #313244;
    max-width: 480px;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h1 { font-size: 20px; font-weight: 600; margin: 0; }
  p { margin: 0; color: #a6adc8; font-size: 14px; }
  input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
  }
  input:focus { border-color: #89b4fa; }
  button {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px 20px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled { opacity: 0.5; cursor: not-allowed; }
  .success { color: #a6e3a1; }
  .warning { color: #fab387; }
  .hint { font-size: 13px; }
  code {
    background: #313244;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 13px;
  }
</style>
```

**Step 3: Verify**

Run: `npm run build`
Expected: Compiles.

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: add onboarding component with projects root and claude CLI check"
```

---

### Task 5: Fuzzy Finder Modal Component

**Files:**
- Create: `src/lib/FuzzyFinder.svelte`

**Step 1: Create FuzzyFinder.svelte**

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "./toast";

  interface Props {
    onSelect: (entry: { name: string; path: string }) => void;
    onClose: () => void;
  }

  let { onSelect, onClose }: Props = $props();

  interface DirEntry {
    name: string;
    path: string;
  }

  let query = $state("");
  let entries = $state<DirEntry[]>([]);
  let filtered = $derived(
    query.trim() === ""
      ? entries
      : entries.filter((e) =>
          e.name.toLowerCase().includes(query.toLowerCase())
        )
  );
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(async () => {
    try {
      entries = await invoke<DirEntry[]>("list_root_directories");
    } catch (e) {
      showToast(String(e), "error");
    }
    inputEl?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === "Enter" && filtered.length > 0) {
      e.preventDefault();
      onSelect(filtered[selectedIndex]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  // Reset selection when query changes
  $effect(() => {
    query;
    selectedIndex = 0;
  });
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <div class="modal" onclick|stopPropagation={() => {}}>
    <input
      bind:this={inputEl}
      bind:value={query}
      placeholder="Search projects..."
      class="search-input"
      onkeydown={handleKeydown}
    />
    <div class="results">
      {#each filtered as entry, i (entry.path)}
        <div
          class="result-item"
          class:selected={i === selectedIndex}
          onclick={() => onSelect(entry)}
          role="option"
          aria-selected={i === selectedIndex}
        >
          <span class="entry-name">{entry.name}</span>
          <span class="entry-path">{entry.path}</span>
        </div>
      {/each}
      {#if filtered.length === 0}
        <div class="empty">No matching directories</div>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 500px;
    max-height: 400px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .search-input {
    background: #1e1e2e;
    color: #cdd6f4;
    border: none;
    border-bottom: 1px solid #313244;
    padding: 14px 16px;
    font-size: 15px;
    outline: none;
  }
  .results {
    overflow-y: auto;
    max-height: 300px;
  }
  .result-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    cursor: pointer;
  }
  .result-item:hover,
  .result-item.selected {
    background: #313244;
  }
  .entry-name {
    color: #cdd6f4;
    font-size: 14px;
  }
  .entry-path {
    color: #6c7086;
    font-size: 12px;
  }
  .empty {
    padding: 20px 16px;
    color: #6c7086;
    font-size: 13px;
    text-align: center;
  }
</style>
```

**Step 2: Verify**

Run: `npm run build`
Expected: Compiles.

**Step 3: Commit**

```bash
git add src/
git commit -m "feat: add fuzzy finder modal component"
```

---

### Task 6: New Project Modal Component

**Files:**
- Create: `src/lib/NewProjectModal.svelte`

**Step 1: Create NewProjectModal.svelte**

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { showToast } from "./toast";
  import type { Project } from "./stores";

  interface Props {
    onCreated: (project: Project) => void;
    onClose: () => void;
  }

  let { onCreated, onClose }: Props = $props();

  let step = $state<"describe" | "pick">("describe");
  let description = $state("");
  let suggestions = $state<string[]>([]);
  let customName = $state("");
  let loading = $state(false);
  let selectedIndex = $state(0);

  async function generateNames() {
    if (!description.trim()) return;
    loading = true;
    try {
      suggestions = await invoke<string[]>("generate_project_names", {
        description: description.trim(),
      });
      step = "pick";
      selectedIndex = 0;
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      loading = false;
    }
  }

  async function createWithName(name: string) {
    if (!name.trim()) return;
    loading = true;
    try {
      const project = await invoke<Project>("scaffold_project", {
        name: name.trim(),
      });
      onCreated(project);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (step === "describe" && e.key === "Enter") {
      e.preventDefault();
      generateNames();
    } else if (step === "pick") {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, suggestions.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (selectedIndex < suggestions.length) {
          createWithName(suggestions[selectedIndex]);
        } else if (customName.trim()) {
          createWithName(customName);
        }
      }
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <div class="modal" onclick|stopPropagation={() => {}}>
    {#if step === "describe"}
      <div class="modal-header">New Project</div>
      <p class="hint">Describe your project in a few words</p>
      <input
        bind:value={description}
        placeholder="e.g. real-time chat app"
        class="input"
        disabled={loading}
        onkeydown={handleKeydown}
      />
      <button class="btn-primary" onclick={generateNames} disabled={!description.trim() || loading}>
        {loading ? "Generating..." : "Generate Names"}
      </button>
    {:else}
      <div class="modal-header">Pick a name</div>
      <div class="suggestions">
        {#each suggestions as name, i}
          <div
            class="suggestion"
            class:selected={i === selectedIndex}
            onclick={() => createWithName(name)}
            role="option"
            aria-selected={i === selectedIndex}
          >
            {name}
          </div>
        {/each}
      </div>
      <div class="custom-name">
        <input
          bind:value={customName}
          placeholder="Or type a custom name..."
          class="input"
          class:selected={selectedIndex === suggestions.length}
          onfocus={() => (selectedIndex = suggestions.length)}
          onkeydown={handleKeydown}
        />
      </div>
      <div class="actions">
        <button class="btn-secondary" onclick={() => (step = "describe")}>Back</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .hint { color: #a6adc8; font-size: 13px; margin: 0; }
  .input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus { border-color: #89b4fa; }
  .btn-primary {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-secondary {
    background: #45475a;
    color: #cdd6f4;
    border: none;
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  .suggestions { display: flex; flex-direction: column; gap: 4px; }
  .suggestion {
    padding: 10px 12px;
    border-radius: 6px;
    cursor: pointer;
    color: #cdd6f4;
    font-size: 14px;
    font-family: monospace;
  }
  .suggestion:hover, .suggestion.selected { background: #313244; }
  .custom-name { margin-top: 4px; }
  .actions { display: flex; justify-content: flex-start; }
</style>
```

**Step 2: Verify**

Run: `npm run build`
Expected: Compiles.

**Step 3: Commit**

```bash
git add src/
git commit -m "feat: add new project modal with LLM name generation"
```

---

### Task 7: Wire Everything Together

**Files:**
- Modify: `src/App.svelte`
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src/lib/stores.ts`

**Step 1: Update App.svelte for conditional onboarding**

Replace `src/App.svelte`:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Toast from "./lib/Toast.svelte";
  import { appConfig, onboardingComplete, type Config } from "./lib/stores";

  let ready = $state(false);
  let needsOnboarding = $state(true);

  onMount(async () => {
    try {
      const config = await invoke<Config | null>("check_onboarding");
      if (config) {
        appConfig.set(config);
        onboardingComplete.set(true);
        needsOnboarding = false;
      }
    } catch (e) {
      // Config check failed, show onboarding
    }
    ready = true;
  });

  // Listen for onboarding completion
  onboardingComplete.subscribe((complete) => {
    if (complete) needsOnboarding = false;
  });
</script>

{#if ready}
  {#if needsOnboarding}
    <Onboarding />
  {:else}
    <div class="app-layout">
      <Sidebar />
      <main class="terminal-area">
        <TerminalManager />
      </main>
    </div>
  {/if}
{/if}
<Toast />

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    background: #11111b;
    overflow: hidden;
  }
  .terminal-area {
    flex: 1;
    overflow: hidden;
  }
</style>
```

**Step 2: Update Sidebar.svelte**

Replace the new-project form with modal triggers. Remove `newProjectName`, `newProjectRepoPath`, `newMode` state. Replace with:

```typescript
let showFuzzyFinder = $state(false);
let showNewProjectModal = $state(false);
```

Replace the `showNewProjectForm` toggle area with a dropdown:

```svelte
{#if showNewMenu}
  <div class="new-menu">
    <button onclick={() => { showNewMenu = false; showNewProjectModal = true; }}>Create New</button>
    <button onclick={() => { showNewMenu = false; showFuzzyFinder = true; }}>Load Existing</button>
  </div>
{/if}
```

Add modal rendering at the end of the component:

```svelte
{#if showFuzzyFinder}
  <FuzzyFinder
    onSelect={async (entry) => {
      showFuzzyFinder = false;
      try {
        await invoke("load_project", { name: entry.name, repoPath: entry.path });
        await loadProjects();
      } catch (e) {
        showToast(String(e), "error");
      }
    }}
    onClose={() => (showFuzzyFinder = false)}
  />
{/if}

{#if showNewProjectModal}
  <NewProjectModal
    onCreated={async () => {
      showNewProjectModal = false;
      await loadProjects();
    }}
    onClose={() => (showNewProjectModal = false)}
  />
{/if}
```

Import the new components:

```typescript
import FuzzyFinder from "./FuzzyFinder.svelte";
import NewProjectModal from "./NewProjectModal.svelte";
```

Remove the old form HTML and its CSS. Remove `createProject` and `loadExistingProject` functions. Remove the `newProjectName`, `newProjectRepoPath`, `newMode`, `showNewProjectForm` state variables.

Update `createSession` to not pass `label`:

```typescript
const sessionId = await invoke<string>("create_session", { projectId });
```

**Step 3: Verify full build**

Run: `cd src-tauri && cargo build`
Expected: Compiles.

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

Run: `npm run build`
Expected: Frontend compiles.

**Step 4: Commit**

```bash
git add src/ src-tauri/
git commit -m "feat: wire onboarding, fuzzy finder, and new project modal into app"
```
