# Assign GitHub Issue to Session — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users assign a GitHub issue to a session at creation time, displaying the link in the sidebar and posting a comment on the issue.

**Architecture:** Add `github_issue` field to `SessionConfig` (Rust + TS). Change `c` hotkey to open an issue picker modal; `C` keeps current raw-session behavior. On session creation with an issue, post a comment via `gh` CLI. Pass issue context to Claude via `--prompt`.

**Tech Stack:** Rust (Tauri v2), Svelte 5, `gh` CLI, tmux

---

### Task 1: Add `github_issue` field to `SessionConfig` (backend)

**Files:**
- Modify: `src-tauri/src/models.rs:14-24` (SessionConfig struct)
- Test: `src-tauri/src/models.rs:68-156` (existing tests)

**Step 1: Write the failing test**

Add to `src-tauri/src/models.rs` tests module:

```rust
#[test]
fn test_session_config_github_issue_roundtrip() {
    let session = SessionConfig {
        id: Uuid::new_v4(),
        label: "session-1".to_string(),
        worktree_path: None,
        worktree_branch: None,
        archived: false,
        kind: "claude".to_string(),
        github_issue: Some(GithubIssue {
            number: 22,
            title: "Assign GitHub issue to a session".to_string(),
            url: "https://github.com/kwannoel/the-controller/issues/22".to_string(),
            labels: vec![],
        }),
    };
    let json = serde_json::to_string(&session).expect("serialize");
    let deserialized: SessionConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.github_issue.as_ref().unwrap().number, 22);
    assert_eq!(deserialized.github_issue.as_ref().unwrap().title, "Assign GitHub issue to a session");
}

#[test]
fn test_session_config_github_issue_defaults_to_none() {
    let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false}"#;
    let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
    assert!(session.github_issue.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_session_config_github_issue`
Expected: FAIL — `SessionConfig` has no `github_issue` field

**Step 3: Write minimal implementation**

In `src-tauri/src/models.rs`, add to `SessionConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: Uuid,
    pub label: String,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub github_issue: Option<GithubIssue>,
}
```

**Step 4: Update all `SessionConfig` construction sites**

Add `github_issue: None` to every place that constructs a `SessionConfig`:
- `src-tauri/src/commands.rs:418-425` (create_session)
- `src-tauri/src/models.rs` test helpers (test_project_serialization_roundtrip, test_project_with_worktree_session)

**Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands.rs
git commit -m "feat: add github_issue field to SessionConfig (#22)"
```

---

### Task 2: Add `post_github_comment` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs` (add new command)
- Modify: `src-tauri/src/lib.rs:23-55` (register command)

**Step 1: Write the command**

Add to `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn post_github_comment(
    repo_path: String,
    issue_number: u64,
    body: String,
) -> Result<(), String> {
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "comment",
            &issue_number.to_string(),
            "--repo", &nwo,
            "--body", &body,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue comment failed: {}", stderr));
    }

    Ok(())
}
```

**Step 2: Register in lib.rs**

Add `commands::post_github_comment,` to the `invoke_handler` list in `src-tauri/src/lib.rs`.

**Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add post_github_comment Tauri command (#22)"
```

---

### Task 3: Accept `github_issue` in `create_session` command

**Files:**
- Modify: `src-tauri/src/commands.rs:367-434` (create_session fn)

**Step 1: Update `create_session` signature and body**

Add `github_issue: Option<crate::models::GithubIssue>` parameter. When constructing `SessionConfig`, use the passed value instead of `None`:

```rust
#[tauri::command]
pub fn create_session(
    state: State<AppState>,
    app_handle: AppHandle,
    project_id: String,
    kind: Option<String>,
    github_issue: Option<crate::models::GithubIssue>,
) -> Result<String, String> {
    // ... existing code ...

    let session_config = SessionConfig {
        id: session_id,
        label: label.clone(),
        worktree_path: wt_path,
        worktree_branch: wt_branch,
        archived: false,
        kind: kind.clone(),
        github_issue,
    };
    // ... rest unchanged ...
}
```

**Step 2: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: PASS (existing callers pass no `github_issue` arg — Tauri deserializes missing optional fields as `None`)

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: accept github_issue param in create_session (#22)"
```

---

### Task 4: Add `github_issue` to frontend `SessionConfig` type

**Files:**
- Modify: `src/lib/stores.ts:3-9` (SessionConfig interface)

**Step 1: Update the interface**

```typescript
export interface GithubIssue {
  number: number;
  title: string;
  url: string;
  labels: { name: string }[];
}

export interface SessionConfig {
  id: string;
  label: string;
  worktree_path: string | null;
  worktree_branch: string | null;
  archived: boolean;
  github_issue: GithubIssue | null;
}
```

**Step 2: Verify no type errors**

Run: `npx tsc --noEmit` or `npm run check` (if available)
Expected: PASS

**Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: add GithubIssue type and field to frontend SessionConfig (#22)"
```

---

### Task 5: Create `IssuePickerModal.svelte`

**Files:**
- Create: `src/lib/IssuePickerModal.svelte`

**Step 1: Create the component**

Model after `CreateIssueModal.svelte` styling. Lists issues fetched via `list_github_issues`. User clicks an issue to select it. Include a loading state. Escape closes. Also show a "Skip — create raw session" option at the bottom.

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  interface GithubIssue {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

  interface Props {
    repoPath: string;
    onSelect: (issue: GithubIssue) => void;
    onSkip: () => void;
    onClose: () => void;
  }

  let { repoPath, onSelect, onSkip, onClose }: Props = $props();

  let issues: GithubIssue[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      issues = await invoke<GithubIssue[]>("list_github_issues", { repoPath });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">Assign Issue to New Session</div>
    {#if loading}
      <div class="status">Loading issues...</div>
    {:else if error}
      <div class="status error">{error}</div>
    {:else if issues.length === 0}
      <div class="status">No open issues</div>
    {:else}
      <ul class="issue-list">
        {#each issues as issue}
          <li>
            <button class="issue-btn" onclick={() => onSelect(issue)}>
              <span class="issue-number">#{issue.number}</span>
              <span class="issue-title">{issue.title}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    <button class="btn-skip" onclick={onSkip}>
      Skip — create raw session
    </button>
  </div>
</div>
```

Style with Catppuccin Mocha, matching `CreateIssueModal.svelte`:
- `.overlay`: fixed inset, dark backdrop, centered at 20vh
- `.modal`: `#1e1e2e` bg, `#313244` border, 420px width, 24px padding
- `.issue-list`: scrollable, max-height ~50vh
- `.issue-btn`: full-width, flex row, hover `#313244`
- `.issue-number`: `#89b4fa`, no-wrap
- `.issue-title`: `#cdd6f4`, truncate with ellipsis
- `.btn-skip`: subtle secondary style, `#6c7086` text, border `#45475a`

**Step 2: Verify it renders**

Manual: `npm run tauri dev`, open modal. (Wiring comes in Task 6.)

**Step 3: Commit**

```bash
git add src/lib/IssuePickerModal.svelte
git commit -m "feat: create IssuePickerModal component (#22)"
```

---

### Task 6: Wire up hotkeys and App orchestration

**Files:**
- Modify: `src/lib/stores.ts:32-47` (HotkeyAction type)
- Modify: `src/lib/HotkeyManager.svelte:341-347` (c/C hotkey)
- Modify: `src/App.svelte` (import modal, handle flow)
- Modify: `src/lib/Sidebar.svelte:147-155` (create-session action handler)

**Step 1: Add new hotkey action type**

In `src/lib/stores.ts`, add to `HotkeyAction`:

```typescript
| { type: "pick-issue-for-session"; projectId: string; repoPath: string }
```

**Step 2: Change `c` / `C` hotkey behavior**

In `src/lib/HotkeyManager.svelte`, update the `case "c":` block:

```typescript
case "c":
  if (currentFocus?.type === "project" || currentFocus?.type === "session") {
    const project = projectList.find(p => p.id === currentFocus.projectId);
    if (project) {
      dispatchAction({ type: "pick-issue-for-session", projectId: project.id, repoPath: project.repo_path });
    }
  }
  return true;
case "C":
  if (currentFocus?.type === "project" || currentFocus?.type === "session") {
    dispatchAction({ type: "create-session", projectId: currentFocus.projectId });
  }
  return true;
```

**Step 3: Handle in App.svelte**

Import `IssuePickerModal`. Add state for the picker target. In the `hotkeyAction` subscriber, handle `pick-issue-for-session`. Orchestrate the flow:

```typescript
// State
let issuePickerTarget: { projectId: string; repoPath: string } | null = $state(null);

// In hotkeyAction subscriber:
} else if (action?.type === "pick-issue-for-session") {
  issuePickerTarget = { projectId: action.projectId, repoPath: action.repoPath };
}

// Handlers
function handleIssuePicked(issue: GithubIssue) {
  const target = issuePickerTarget!;
  issuePickerTarget = null;
  createSessionWithIssue(target.projectId, target.repoPath, issue);
}

function handleIssuePickerSkip() {
  const target = issuePickerTarget!;
  issuePickerTarget = null;
  hotkeyAction.set({ type: "create-session", projectId: target.projectId });
  setTimeout(() => hotkeyAction.set(null), 0);
}

async function createSessionWithIssue(projectId: string, repoPath: string, issue: GithubIssue) {
  try {
    const sessionId: string = await invoke("create_session", {
      projectId,
      githubIssue: issue,
    });
    // Post comment on the issue (fire and forget — don't block session start)
    invoke("post_github_comment", {
      repoPath,
      issueNumber: issue.number,
      body: `Working on this in session \`${sessionId.substring(0, 8)}\``,
    }).catch((e: unknown) => showToast(`Failed to post comment: ${e}`, "error"));

    sessionStatuses.update(m => {
      const next = new Map(m);
      next.set(sessionId, "working");
      return next;
    });
    activeSessionId.set(sessionId);
    await invoke("list_projects").then((result: unknown) => projects.set(result as Project[]));
    expandedProjects.update(s => { const next = new Set(s); next.add(projectId); return next; });
    setTimeout(() => {
      hotkeyAction.set({ type: "focus-terminal" });
      setTimeout(() => hotkeyAction.set(null), 0);
    }, 50);
  } catch (e) {
    showToast(String(e), "error");
  }
}
```

In the template, add the modal:

```svelte
{#if issuePickerTarget}
  <IssuePickerModal
    repoPath={issuePickerTarget.repoPath}
    onSelect={handleIssuePicked}
    onSkip={handleIssuePickerSkip}
    onClose={() => { issuePickerTarget = null; }}
  />
{/if}
```

**Step 4: Verify**

Manual: `npm run tauri dev`
- Focus a project, press `c` → issue picker opens
- Select an issue → session spawns, comment posted on issue
- Press `C` → raw session spawns (old behavior)
- Press Escape in picker → modal closes

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/HotkeyManager.svelte src/App.svelte
git commit -m "feat: wire c hotkey to issue picker, C for raw session (#22)"
```

---

### Task 7: Show issue badge in sidebar

**Files:**
- Modify: `src/lib/Sidebar.svelte:542-565` (session item rendering)

**Step 1: Add issue badge next to session label**

In the active sessions `{#each}` block, after the session label span, add:

```svelte
{#if session.github_issue}
  <span class="issue-badge">#{session.github_issue.number}</span>
{/if}
```

**Step 2: Add CSS**

```css
.issue-badge {
  font-size: 10px;
  color: #89b4fa;
  background: rgba(137, 180, 250, 0.15);
  padding: 0 4px;
  border-radius: 3px;
  white-space: nowrap;
  flex-shrink: 0;
}
```

**Step 3: Verify**

Manual: Create a session with an issue assigned. Verify badge shows in sidebar.

**Step 4: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "feat: show issue badge in sidebar for linked sessions (#22)"
```

---

### Task 8: Update hotkey help

**Files:**
- Modify: `src/lib/HotkeyHelp.svelte:15-16`

**Step 1: Update help text**

Change the `c` entry from:
```typescript
{ key: "c", description: "Create new session in focused project" },
```
to:
```typescript
{ key: "c", description: "Create session with issue (pick from list)" },
{ key: "C", description: "Create raw session (no issue)" },
```

**Step 2: Verify**

Manual: Press `?` to open help, confirm updated descriptions.

**Step 3: Commit**

```bash
git add src/lib/HotkeyHelp.svelte
git commit -m "feat: update hotkey help for c/C session creation (#22)"
```

---

### Task 9: Context injection — pass issue to Claude session

**Files:**
- Modify: `src-tauri/src/tmux.rs:28-68` (create_session)
- Modify: `src-tauri/src/pty_manager.rs:34-57` (spawn_session)
- Modify: `src-tauri/src/commands.rs:367-434` (create_session command)

**Step 1: Thread `initial_prompt` through spawn chain**

Add `initial_prompt: Option<String>` to `PtyManager::spawn_session()`, `TmuxManager::create_session()`, and `PtyManager::spawn_direct_session()`.

When `initial_prompt` is `Some(prompt)`, add `"--prompt"` and the prompt string to the Claude args.

In `commands::create_session`, construct the prompt from the issue:

```rust
let initial_prompt = github_issue.as_ref().map(|issue| {
    format!(
        "You are working on GitHub issue #{}: {}\nIssue URL: {}\nPlease include 'closes #{}' in any PR descriptions or final commit messages.",
        issue.number, issue.title, issue.url, issue.number
    )
});
```

Pass it through: `pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle, false, initial_prompt)?;`

**Step 2: Update TmuxManager::create_session signature**

```rust
pub fn create_session(
    session_id: Uuid,
    working_dir: &str,
    command: &str,
    continue_session: bool,
    initial_prompt: Option<&str>,
) -> Result<(), String> {
    // ... existing setup ...
    if command == "claude" {
        args.push("--settings");
        args.push(&settings_json);
        if let Some(prompt) = initial_prompt {
            args.push("--prompt");
            args.push(prompt);
        }
    }
    // ... rest unchanged ...
}
```

Do the same for `spawn_direct_session`.

**Step 3: Update all call sites of spawn_session**

- `commands::create_session` — pass `initial_prompt`
- `commands::restore_sessions` (search for `spawn_session` calls) — pass `None`
- `commands::unarchive_session` (if it calls spawn_session) — pass `None`

Run: `cd src-tauri && cargo check`

**Step 4: Verify**

Manual: Create a session with an issue. Verify Claude starts with the issue context visible.

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/pty_manager.rs src-tauri/src/tmux.rs
git commit -m "feat: inject issue context as initial prompt to Claude (#22)"
```

---

### Task 10: Final verification and cleanup

**Step 1: Run all tests**

```bash
cd src-tauri && cargo test
npx vitest run
```

**Step 2: Manual smoke test**

1. Focus a project with open GitHub issues
2. Press `c` → issue picker modal opens with issues listed
3. Select an issue → session spawns, sidebar shows `#N` badge
4. Check GitHub issue page → comment posted
5. Claude session starts with issue context
6. Press `C` → raw session spawns (no picker)
7. Press `c` then Escape → modal closes, no session created
8. Press `c` then "Skip" → raw session spawns

**Step 3: Commit any fixes, then final commit if needed**
