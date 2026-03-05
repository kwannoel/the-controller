# Fix GitHub Issue Creation Implementation Plan (#31)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix issue creation to be non-blocking and remove the unsupported `--json` flag from `gh issue create`.

**Architecture:** Split backend into two commands (`generate_issue_body` + `create_github_issue` taking a body param). Frontend closes modal immediately on submit and runs async flow with toast notifications for each step. Parse issue URL instead of using `--json`.

**Tech Stack:** Rust/Tauri backend, Svelte 5 frontend (runes syntax), `gh` CLI, `claude` CLI

---

### Task 1: Add `parse_github_issue_url` helper and test

**Files:**
- Modify: `src-tauri/src/commands.rs` (add helper near `parse_github_nwo` ~line 689, add tests ~line 1057)

**Step 1: Write the failing test**

Add after the existing `parse_github_nwo` tests (around line 1057):

```rust
// --- parse_github_issue_url tests ---

#[test]
fn test_parse_github_issue_url_basic() {
    assert_eq!(
        parse_github_issue_url("https://github.com/owner/repo/issues/42").unwrap(),
        42
    );
}

#[test]
fn test_parse_github_issue_url_trailing_newline() {
    assert_eq!(
        parse_github_issue_url("https://github.com/owner/repo/issues/7\n").unwrap(),
        7
    );
}

#[test]
fn test_parse_github_issue_url_invalid() {
    assert!(parse_github_issue_url("not a url").is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-1 && cargo test -p the-controller parse_github_issue_url`
Expected: FAIL — function not found

**Step 3: Write minimal implementation**

Add near `parse_github_nwo` (around line 689):

```rust
/// Parse a GitHub issue URL like "https://github.com/owner/repo/issues/42" and return the issue number.
fn parse_github_issue_url(url: &str) -> Result<u64, String> {
    let url = url.trim();
    let parts: Vec<&str> = url.rsplitn(2, '/').collect();
    if parts.len() == 2 {
        if let Ok(num) = parts[0].parse::<u64>() {
            return Ok(num);
        }
    }
    Err(format!("Could not parse issue number from URL: {}", url))
}
```

**Step 4: Run test to verify it passes**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-1 && cargo test -p the-controller parse_github_issue_url`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat(#31): add parse_github_issue_url helper with tests"
```

---

### Task 2: Add `generate_issue_body` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs` (add new command before `create_github_issue` ~line 750)
- Modify: `src-tauri/src/lib.rs` (register command ~line 52)

**Step 1: Add the new command**

Add before `create_github_issue` (around line 750):

```rust
#[tauri::command]
pub async fn generate_issue_body(title: String) -> Result<String, String> {
    let prompt = format!(
        "Write a concise GitHub issue body for an issue titled: \"{}\". \
         Include a Summary section and a Details section. \
         Keep it under 200 words. Return only the markdown body, nothing else.",
        title
    );
    let output = tokio::process::Command::new("claude")
        .args(["--print", &prompt])
        .env_remove("CLAUDECODE")
        .output()
        .await
        .map_err(|e| format!("Failed to run claude: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}
```

**Step 2: Register the command in lib.rs**

In `src-tauri/src/lib.rs`, add `commands::generate_issue_body,` after `commands::list_github_issues,` (line 52).

**Step 3: Verify it compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-1 && cargo check -p the-controller`
Expected: compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(#31): add generate_issue_body Tauri command"
```

---

### Task 3: Fix `create_github_issue` — remove `--json`, accept body param, parse URL

**Files:**
- Modify: `src-tauri/src/commands.rs` (rewrite `create_github_issue` ~lines 750-805)

**Step 1: Rewrite the command**

Replace the existing `create_github_issue` function with:

```rust
#[tauri::command]
pub async fn create_github_issue(
    repo_path: String,
    title: String,
    body: String,
) -> Result<crate::models::GithubIssue, String> {
    // Step 1: Extract GitHub owner/repo
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    // Step 2: Create the issue via gh CLI (no --json flag)
    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "create",
            "--repo", &nwo,
            "--title", &title,
            "--body", &body,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue create failed: {}", stderr));
    }

    // Step 3: Parse the issue URL from stdout
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let number = parse_github_issue_url(&url)?;

    Ok(crate::models::GithubIssue {
        number,
        title,
        url,
        labels: vec![],
    })
}
```

**Step 2: Verify it compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-1 && cargo check -p the-controller`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix(#31): remove --json flag from gh issue create, accept body param, parse URL"
```

---

### Task 4: Update frontend — close modal immediately, async flow with toasts

**Files:**
- Modify: `src/lib/CreateIssueModal.svelte` (simplify to just submit title)
- Modify: `src/App.svelte` (async flow with toasts)

**Step 1: Simplify CreateIssueModal.svelte**

Replace the `<script>` section entirely:

```typescript
<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    onSubmit: (title: string) => void;
    onClose: () => void;
  }

  let { onSubmit, onClose }: Props = $props();

  let title = $state("");
  let titleInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  function submit() {
    if (!title.trim()) return;
    onSubmit(title.trim());
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  }
</script>
```

Update the template:
- Remove `disabled={loading}` from the input element
- Change button to: `<button class="btn-primary" onclick={submit} disabled={!title.trim()}>Create Issue</button>`

**Step 2: Update App.svelte**

Add import for `showToast`:
```typescript
import { showToast } from "./lib/toast";
```

Add `GithubIssue` interface in the script section:
```typescript
interface GithubIssue {
  number: number;
  title: string;
  url: string;
  labels: { name: string }[];
}
```

Replace `handleIssueCreated` with:

```typescript
async function handleIssueSubmit(title: string) {
  const repoPath = createIssueTarget!.repoPath;
  createIssueTarget = null; // close modal immediately

  try {
    showToast("Generating issue description...", "info");
    const body = await invoke<string>("generate_issue_body", { title });

    showToast("Creating issue...", "info");
    const issue = await invoke<GithubIssue>("create_github_issue", {
      repoPath,
      title,
      body,
    });

    showToast(`Issue #${issue.number} created`, "info");
    taskPanelVisible.set(true);
    setTimeout(() => {
      taskPanelRef?.insertIssue(issue);
    }, 50);
  } catch (e) {
    showToast(String(e), "error");
  }
}
```

Update the template — change `CreateIssueModal` usage:
- Remove `repoPath` prop
- Change `onCreated={handleIssueCreated}` to `onSubmit={handleIssueSubmit}`

**Step 3: Verify it compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-1 && npm run check`
Expected: no type errors

**Step 4: Commit**

```bash
git add src/lib/CreateIssueModal.svelte src/App.svelte
git commit -m "feat(#31): close modal immediately, async issue creation with toast notifications"
```

---

### Task 5: Manual validation

**Step 1: Build and run the app**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-1 && cargo tauri dev`

**Step 2: Test the happy path**

1. Focus a project, press `i`
2. Type a title, press Enter
3. Verify: modal closes immediately
4. Verify: toast "Generating issue description..." appears
5. Verify: toast "Creating issue..." appears
6. Verify: toast "Issue #N created" appears
7. Verify: issue appears in TaskPanel

**Step 3: Test error handling**

1. Disconnect network, try creating an issue
2. Verify: error toast appears
