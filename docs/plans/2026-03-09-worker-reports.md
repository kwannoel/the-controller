# Worker Issue Reports Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Workers post report comments on GitHub issues after merging, and the dashboard displays those reports.

**Architecture:** Three changes: (1) update the worker prompt to post a report comment, (2) add `has_merged_pr_sync` check in the scheduler before labeling `finished-by-worker`, (3) add a `get_worker_reports` Tauri command and display reports in the auto-worker dashboard.

**Tech Stack:** Rust (Tauri commands, `std::process::Command` for `gh`), Svelte 5 (dashboard UI)

---

### Task 1: Add report step to worker prompt

**Files:**
- Modify: `src-tauri/src/session_args.rs:3` (the `BACKGROUND_WORKFLOW_SUFFIX` constant)
- Test: `src-tauri/src/session_args.rs:148-161` (existing `build_issue_prompt_with_background` test)

**Step 1: Update the prompt constant**

In `src-tauri/src/session_args.rs`, update `BACKGROUND_WORKFLOW_SUFFIX` to insert a new step between "Merge" (step 5) and "Sync local master" (step 6). The new numbering becomes:

```rust
// In the BACKGROUND_WORKFLOW_SUFFIX string, after step 5 (Merge) and before step 6 (Sync):
// Add:
// 6. **Report** — After a successful merge, post a report comment on the issue via `gh issue comment <number> --body "..."`. The report should summarize: what was changed, the PR URL, and that the merge succeeded. Only post if the PR was actually merged.
// 7. **Sync local master** — (existing step 6, renumbered)
```

The full updated constant should have steps: Design, Implement, Review, Push PR, Merge, Report, Sync local master.

**Step 2: Update the test**

Add an assertion to the existing `build_issue_prompt_with_background` test:

```rust
assert!(prompt.contains("Report"));
assert!(prompt.contains("gh issue comment"));
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test --lib session_args`
Expected: all 10 tests pass

**Step 4: Commit**

```bash
git add src-tauri/src/session_args.rs
git commit -m "feat: add report step to worker prompt"
```

---

### Task 2: Add merged PR verification to scheduler

**Files:**
- Modify: `src-tauri/src/auto_worker.rs:325-328` (`mark_issue_finished` function)
- Test: `src-tauri/src/auto_worker.rs:363` (tests module)

**Step 1: Write the failing test**

Add to the tests module in `auto_worker.rs`. Since `has_merged_pr_sync` shells out to `gh`, we test it as a pure function that parses output. Add a helper `parse_merged_pr_count` that takes the JSON output string and returns whether a merged PR exists:

```rust
#[test]
fn parse_merged_pr_count_with_result() {
    let json = r#"[{"number":42}]"#;
    assert!(parse_merged_pr_count(json));
}

#[test]
fn parse_merged_pr_count_empty() {
    let json = "[]";
    assert!(!parse_merged_pr_count(json));
}

#[test]
fn parse_merged_pr_count_invalid_json() {
    assert!(!parse_merged_pr_count("not json"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib auto_worker`
Expected: FAIL — `parse_merged_pr_count` not defined

**Step 3: Implement `parse_merged_pr_count` and `has_merged_pr_sync`**

Add above `mark_issue_finished` in `auto_worker.rs`:

```rust
fn parse_merged_pr_count(json: &str) -> bool {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

fn has_merged_pr_sync(repo_path: &str, issue_number: u64) -> bool {
    let search_query = format!("closes #{}", issue_number);
    let output = Command::new("gh")
        .args([
            "pr", "list",
            "--search", &search_query,
            "--state", "merged",
            "--json", "number",
            "--limit", "1",
        ])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            parse_merged_pr_count(&String::from_utf8_lossy(&o.stdout))
        }
        _ => false,
    }
}
```

**Step 4: Update `mark_issue_finished` to gate on merged PR**

Replace the current `mark_issue_finished`:

```rust
fn mark_issue_finished(session: &ActiveSession) {
    if has_merged_pr_sync(&session.repo_path, session.issue_number) {
        let _ = add_label_sync(&session.repo_path, session.issue_number, "finished-by-worker");
    }
    let _ = remove_label_sync(&session.repo_path, session.issue_number, "in-progress");
}
```

**Step 5: Run tests**

Run: `cd src-tauri && cargo test --lib auto_worker`
Expected: all tests pass (existing + 3 new)

**Step 6: Commit**

```bash
git add src-tauri/src/auto_worker.rs
git commit -m "feat: verify merged PR before labeling finished-by-worker"
```

---

### Task 3: Add `get_worker_reports` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/github.rs` (add new async function)
- Modify: `src-tauri/src/commands.rs` (re-export the command)
- Modify: `src-tauri/src/lib.rs:77` (register in invoke_handler)

**Step 1: Define the return type**

Add a `WorkerReport` struct at the top of `src-tauri/src/commands/github.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerReport {
    pub issue_number: u64,
    pub title: String,
    pub comment_body: String,
    pub updated_at: String,
}
```

**Step 2: Implement the command**

Add to `src-tauri/src/commands/github.rs`:

```rust
pub(crate) async fn get_worker_reports(repo_path: String) -> Result<Vec<WorkerReport>, String> {
    let nwo = extract_github_repo_async(repo_path).await?;

    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "list",
            "--repo", &nwo,
            "--label", "finished-by-worker",
            "--state", "all",
            "--json", "number,title,comments,updatedAt",
            "--limit", "50",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    let reports: Vec<WorkerReport> = raw
        .into_iter()
        .filter_map(|issue| {
            let number = issue["number"].as_u64()?;
            let title = issue["title"].as_str()?.to_string();
            let updated_at = issue["updatedAt"].as_str().unwrap_or("").to_string();
            let comments = issue["comments"].as_array()?;
            let last_comment = comments.last()?;
            let body = last_comment["body"].as_str().unwrap_or("").to_string();
            Some(WorkerReport {
                issue_number: number,
                title,
                comment_body: body,
                updated_at,
            })
        })
        .collect();

    Ok(reports)
}
```

**Step 3: Wire up the command**

In `src-tauri/src/commands.rs`, add the public re-export wrapper. Find `configure_auto_worker` (line 1369) and add after it:

```rust
#[tauri::command]
pub async fn get_worker_reports(repo_path: String) -> Result<Vec<github::WorkerReport>, String> {
    github::get_worker_reports(repo_path).await
}
```

In `src-tauri/src/lib.rs`, add `commands::get_worker_reports,` after `commands::configure_auto_worker,` (line 77).

**Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib`
Expected: compiles and all tests pass

**Step 5: Commit**

```bash
git add src-tauri/src/commands/github.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add get_worker_reports Tauri command"
```

---

### Task 4: Add WorkerReport type and fetch logic to frontend

**Files:**
- Modify: `src/lib/stores.ts` (add `WorkerReport` interface)
- Modify: `src/lib/AgentDashboard.svelte` (add fetch logic and state)

**Step 1: Add the TypeScript interface**

In `src/lib/stores.ts`, after the `AutoWorkerStatus` type (line 125), add:

```typescript
export interface WorkerReport {
  issue_number: number;
  title: string;
  comment_body: string;
  updated_at: string;
}
```

**Step 2: Add fetch logic to AgentDashboard.svelte**

In `AgentDashboard.svelte`, add state and fetch function:

```typescript
let workerReports: WorkerReport[] = $state([]);
let workerLoading = $state(false);
let workerOpenIndex: number | null = $state(null);
let workerSelectedIndex = $state(0);
```

Add to the `$effect` that watches `prevAgentKey` (line 47-62): when `focusedAgent?.agentKind === "auto-worker"`, call `fetchWorkerReports(pid)`.

```typescript
async function fetchWorkerReports(projectId: string) {
  const proj = projectList.find((p) => p.id === projectId);
  if (!proj) return;
  workerLoading = true;
  try {
    const result = await invoke<WorkerReport[]>("get_worker_reports", { repoPath: proj.repo_path });
    if (prevAgentKey === `${projectId}:auto-worker`) {
      workerReports = result;
    }
  } catch {
    if (prevAgentKey === `${projectId}:auto-worker`) {
      workerReports = [];
    }
  } finally {
    if (prevAgentKey === `${projectId}:auto-worker`) {
      workerLoading = false;
    }
  }
}
```

Add import for `WorkerReport` from stores.

**Step 3: Reset worker state on agent switch**

In the `prevAgentKey` effect (line 47-62), add resets:

```typescript
workerReports = [];
workerOpenIndex = null;
workerSelectedIndex = 0;
```

And trigger fetch:

```typescript
if (pid && focusedAgent?.agentKind === "auto-worker") {
  fetchWorkerReports(pid);
}
```

**Step 4: Commit**

```bash
git add src/lib/stores.ts src/lib/AgentDashboard.svelte
git commit -m "feat: add worker report fetch logic"
```

---

### Task 5: Render worker reports in the dashboard

**Files:**
- Modify: `src/lib/AgentDashboard.svelte:262-284` (auto-worker section)

**Step 1: Add navigation handlers for auto-worker**

Extend `handleNavigate`, `handleSelect`, and `handleEscape` to support auto-worker reports (same pattern as maintainer):

In `handleNavigate`:
```typescript
if (focusedAgent?.agentKind === "auto-worker") {
  if (workerOpenIndex !== null) return; // no sub-navigation needed for single report
  if (workerReports.length === 0) return;
  workerSelectedIndex = Math.max(0, Math.min(workerReports.length - 1, workerSelectedIndex + direction));
}
```

In `handleSelect`:
```typescript
if (focusedAgent?.agentKind === "auto-worker") {
  if (workerOpenIndex !== null) return;
  if (workerReports.length === 0) return;
  workerOpenIndex = workerSelectedIndex;
}
```

In `handleEscape`:
```typescript
// Add before the existing focusedAgent check at line 111:
if (focusedAgent?.agentKind === "auto-worker" && workerOpenIndex !== null) {
  workerOpenIndex = null;
  return;
}
```

**Step 2: Add the reports section to the template**

After the work policy section (line 284), before the `{:else if focusedAgent.agentKind === "maintainer"}` block, add:

```svelte
<section class="section report-section">
  {#if workerLoading}
    <div class="section-body">
      <p class="muted">Loading reports...</p>
    </div>
  {:else if workerOpenIndex !== null && workerReports[workerOpenIndex]}
    {@const report = workerReports[workerOpenIndex]}
    <div class="detail-view">
      <div class="detail-header">
        <span class="detail-back">Reports</span>
        <span class="detail-timestamp">{formatTimestamp(report.updated_at)}</span>
        <span class="detail-summary">#{report.issue_number} {report.title}</span>
      </div>
      <div class="detail-blocks">
        <div class="detail-block">
          <div class="worker-report-body">{report.comment_body}</div>
        </div>
      </div>
    </div>
  {:else}
    <div class="report-list">
      {#if workerReports.length === 0}
        <div class="section-body">
          <p class="muted">No completed work yet</p>
        </div>
      {:else}
        {#each workerReports as report, i}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="report-item"
            class:selected={panelFocused && workerSelectedIndex === i}
            data-report-index={i}
            onclick={() => { workerSelectedIndex = i; workerOpenIndex = i; }}
          >
            <span class="log-dot"></span>
            <span class="report-timestamp">{formatTimestamp(report.updated_at)}</span>
            <span class="report-summary-preview">#{report.issue_number} {report.title}</span>
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</section>

{#if !panelFocused && workerReports.length > 0}
  <div class="panel-hint">
    <span class="muted">Press <kbd>l</kbd> to browse reports</span>
  </div>
{/if}
```

**Step 3: Add the worker-report-body CSS**

Add to the `<style>` block:

```css
.worker-report-body { padding: 12px; font-size: 12px; color: #cdd6f4; white-space: pre-wrap; word-break: break-word; background: rgba(49, 50, 68, 0.2); border-radius: 4px; border-left: 3px solid #a6e3a1; }
```

**Step 4: Run frontend tests**

Run: `cd /path/to/worktree && npx vitest run`
Expected: all tests pass

**Step 5: Commit**

```bash
git add src/lib/AgentDashboard.svelte
git commit -m "feat: render worker reports in dashboard"
```

---

### Task 6: Manual smoke test

**Step 1: Start dev server**

Run: `npm run tauri dev`

**Step 2: Verify**

1. Open a project with auto-worker enabled
2. Navigate to the auto-worker agent in the sidebar (o + w to toggle, or just focus it)
3. Verify the dashboard loads and shows "No completed work yet" if no issues have `finished-by-worker`
4. If there are existing `finished-by-worker` issues, verify reports appear and are navigable with j/k/enter/escape

**Step 3: Commit all uncommitted prompt changes**

Ensure the earlier prompt changes (commit tagging, sync master) are included. If they were on separate commits, they should already be committed.
