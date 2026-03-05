# GitHub Task List Panel Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a toggleable RHS panel (`t` key) showing open GitHub issues for the focused project.

**Architecture:** New `list_github_issues` Tauri command shells out to `gh` CLI, extracting the remote from the repo's git config. New `TaskPanel.svelte` component renders the issue list. A `taskPanelVisible` store controls visibility, toggled via `t` in ambient mode.

**Tech Stack:** Rust (git2 for remote extraction, tokio::process::Command for `gh`), Svelte 5 (runes), Catppuccin Mocha theme.

---

### Task 1: Add `GithubIssue` model and `list_github_issues` command (Rust)

**Files:**
- Modify: `src-tauri/src/models.rs` (add `GithubIssue` struct after line 44)
- Modify: `src-tauri/src/commands.rs` (add `list_github_issues` command)
- Modify: `src-tauri/src/lib.rs:18-46` (register new command in `generate_handler!`)

**Step 1: Add `GithubIssue` model**

In `src-tauri/src/models.rs`, add after `SessionInfo`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub labels: Vec<GithubLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubLabel {
    pub name: String,
}
```

**Step 2: Write the `list_github_issues` command**

In `src-tauri/src/commands.rs`, add at the end (before the `#[cfg(test)]` block at line 682):

```rust
/// Extract the GitHub owner/repo from a local git repository's origin remote.
/// Handles both SSH (git@github.com:owner/repo.git) and HTTPS (https://github.com/owner/repo.git) URLs.
fn extract_github_repo(repo_path: &str) -> Result<String, String> {
    let repo = git2::Repository::discover(repo_path)
        .map_err(|e| format!("Failed to open repo: {}", e))?;
    let remote = repo
        .find_remote("origin")
        .map_err(|_| "No 'origin' remote found".to_string())?;
    let url = remote
        .url()
        .ok_or_else(|| "Origin remote URL is not valid UTF-8".to_string())?;

    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return Ok(rest.trim_end_matches(".git").to_string());
    }
    // HTTPS: https://github.com/owner/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        return Ok(rest.trim_end_matches(".git").to_string());
    }

    Err(format!("Not a GitHub remote URL: {}", url))
}

#[tauri::command]
pub async fn list_github_issues(repo_path: String) -> Result<Vec<crate::models::GithubIssue>, String> {
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "list",
            "--repo", &nwo,
            "--json", "number,title,url,labels",
            "--limit", "50",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let issues: Vec<crate::models::GithubIssue> =
        serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    Ok(issues)
}
```

**Step 3: Register the command**

In `src-tauri/src/lib.rs`, add `commands::list_github_issues,` to the `generate_handler!` macro (after `commands::scaffold_project,` on line 46).

**Step 4: Add test for `extract_github_repo`**

In `src-tauri/src/commands.rs`, add to the existing `mod tests` block:

```rust
#[test]
fn test_extract_github_repo_ssh() {
    // This test requires a real repo with an origin remote.
    // We test the URL parsing logic directly instead.
}

// Test the URL parsing patterns used by extract_github_repo
#[test]
fn test_github_url_parsing_ssh() {
    let url = "git@github.com:owner/repo.git";
    let rest = url.strip_prefix("git@github.com:").unwrap();
    assert_eq!(rest.trim_end_matches(".git"), "owner/repo");
}

#[test]
fn test_github_url_parsing_https() {
    let url = "https://github.com/owner/repo.git";
    let rest = url.strip_prefix("https://github.com/").unwrap();
    assert_eq!(rest.trim_end_matches(".git"), "owner/repo");
}

#[test]
fn test_github_url_parsing_https_no_git_suffix() {
    let url = "https://github.com/owner/repo";
    let rest = url.strip_prefix("https://github.com/").unwrap();
    assert_eq!(rest.trim_end_matches(".git"), "owner/repo");
}
```

**Step 5: Run tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass, including the new URL parsing tests.

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add list_github_issues Tauri command (#12)"
```

---

### Task 2: Add `taskPanelVisible` store and `t` keybinding

**Files:**
- Modify: `src/lib/stores.ts:50` (add store)
- Modify: `src/lib/HotkeyManager.svelte:286-373` (add `t` case in `handleHotkey`)
- Modify: `src/lib/HotkeyHelp.svelte:10-24` (add shortcut entry)

**Step 1: Add the store**

In `src/lib/stores.ts`, add after line 50 (`sidebarVisible`):

```typescript
export const taskPanelVisible = writable<boolean>(false);
```

**Step 2: Add `t` keybinding**

In `src/lib/HotkeyManager.svelte`, add the import of `taskPanelVisible` to the imports from `./stores` (line 11), then add a new case in `handleHotkey` before the `default:` case (before line 370):

```typescript
      case "t":
        taskPanelVisible.update(v => !v);
        return true;
```

**Step 3: Add help entry**

In `src/lib/HotkeyHelp.svelte`, add to the `shortcuts` array (after the `s` / "Toggle sidebar" entry on line 21):

```typescript
    { key: "t", description: "Toggle GitHub issues panel" },
```

**Step 4: Run frontend tests**

Run: `npx vitest run`
Expected: All existing tests pass.

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/HotkeyManager.svelte src/lib/HotkeyHelp.svelte
git commit -m "feat: add taskPanelVisible store and t keybinding (#12)"
```

---

### Task 3: Create `TaskPanel.svelte` component

**Files:**
- Create: `src/lib/TaskPanel.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, type Project, type FocusTarget } from "./stores";

  interface GithubIssue {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

  let issues: GithubIssue[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let currentRepoPath: string | null = $state(null);

  let projectList: Project[] = $state([]);
  let currentFocus: FocusTarget = $state(null);

  $effect(() => {
    const unsub = projects.subscribe((v) => { projectList = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = focusTarget.subscribe((v) => { currentFocus = v; });
    return unsub;
  });

  $effect(() => {
    const projectId = currentFocus?.type === "project"
      ? currentFocus.projectId
      : currentFocus?.type === "session"
        ? currentFocus.projectId
        : null;

    const project = projectId
      ? projectList.find((p) => p.id === projectId)
      : projectList[0] ?? null;

    const repoPath = project?.repo_path ?? null;

    if (repoPath && repoPath !== currentRepoPath) {
      currentRepoPath = repoPath;
      fetchIssues(repoPath);
    }
  });

  async function fetchIssues(repoPath: string) {
    loading = true;
    error = null;
    try {
      issues = await invoke<GithubIssue[]>("list_github_issues", { repoPath });
    } catch (e) {
      error = String(e);
      issues = [];
    } finally {
      loading = false;
    }
  }
</script>

<aside class="task-panel">
  <div class="panel-header">GitHub Issues</div>
  {#if loading}
    <div class="status">Loading...</div>
  {:else if error}
    <div class="status error">{error}</div>
  {:else if issues.length === 0}
    <div class="status">No open issues</div>
  {:else}
    <ul class="issue-list">
      {#each issues as issue}
        <li class="issue-item">
          <span class="issue-number">#{issue.number}</span>
          <span class="issue-title">{issue.title}</span>
        </li>
      {/each}
    </ul>
  {/if}
</aside>

<style>
  .task-panel {
    width: 320px;
    min-width: 320px;
    height: 100vh;
    background: #1e1e2e;
    border-left: 1px solid #313244;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    padding: 12px 16px;
    font-size: 13px;
    font-weight: 600;
    color: #cdd6f4;
    border-bottom: 1px solid #313244;
  }
  .status {
    padding: 16px;
    color: #6c7086;
    font-size: 13px;
  }
  .status.error {
    color: #f38ba8;
  }
  .issue-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
  }
  .issue-item {
    padding: 8px 16px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
    font-size: 13px;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .issue-number {
    color: #89b4fa;
    font-weight: 500;
    white-space: nowrap;
  }
  .issue-title {
    color: #cdd6f4;
  }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/TaskPanel.svelte
git commit -m "feat: create TaskPanel component (#12)"
```

---

### Task 4: Wire `TaskPanel` into `App.svelte` layout

**Files:**
- Modify: `src/App.svelte`

**Step 1: Add import and state**

In `src/App.svelte`, add the import (after line 9):

```typescript
  import TaskPanel from "./lib/TaskPanel.svelte";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, taskPanelVisible, type Config } from "./lib/stores";
```

(Replace the existing import on line 10 to include `taskPanelVisible`.)

Add a new reactive state variable (after line 15):

```typescript
  let taskPanelIsVisible = $state(false);
```

Add a new `$effect` (after the `showKeyHints` effect ending at line 25):

```typescript
  $effect(() => {
    const unsub = taskPanelVisible.subscribe((v) => { taskPanelIsVisible = v; });
    return unsub;
  });
```

**Step 2: Add panel to layout**

In the template, add after `</main>` (after line 72):

```svelte
      {#if taskPanelIsVisible}
        <TaskPanel />
      {/if}
```

**Step 3: Verify manually**

Run: `npm run tauri dev`
- Press `t` → panel should appear on the right
- Press `t` again → panel should hide
- With panel open, navigate to a project → issues should load
- Press `?` → "t" should appear in help

**Step 4: Commit**

```bash
git add src/App.svelte
git commit -m "feat: wire TaskPanel into App layout (#12)"
```

---

### Task 5: Refetch on panel reopen

**Files:**
- Modify: `src/lib/TaskPanel.svelte`

The current `$effect` only fetches when `repoPath` changes. We also need to re-fetch when the panel is reopened (toggled from hidden to visible).

**Step 1: Add `visible` prop and refetch logic**

Change `TaskPanel.svelte` to accept a prop and refetch:

At the top of the script:

```typescript
  interface Props {
    visible: boolean;
  }

  let { visible }: Props = $props();
```

Add an effect that refetches when `visible` becomes true:

```typescript
  $effect(() => {
    if (visible && currentRepoPath) {
      fetchIssues(currentRepoPath);
    }
  });
```

**Step 2: Pass the prop from App.svelte**

In `App.svelte`, change the TaskPanel usage to:

```svelte
        <TaskPanel visible={taskPanelIsVisible} />
```

**Step 3: Commit**

```bash
git add src/lib/TaskPanel.svelte src/App.svelte
git commit -m "feat: refetch issues when task panel reopens (#12)"
```

---

### Task 6: Final verification and cleanup

**Step 1: Run all tests**

```bash
cd src-tauri && cargo test
npx vitest run
```

Expected: All tests pass.

**Step 2: Manual smoke test**

Run: `npm run tauri dev`
1. Open the app, select a project that has a GitHub remote
2. Press `t` — panel opens, issues load
3. Navigate to a different project — issues update
4. Press `t` — panel closes
5. Press `t` — panel reopens, issues refetch
6. Press `?` — help shows `t` shortcut

**Step 3: Final commit (if any cleanup needed)**
