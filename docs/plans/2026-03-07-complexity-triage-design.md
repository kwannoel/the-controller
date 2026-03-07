# Combined Priority + Complexity Triage — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a complexity step (simple/complex) to the triage card flow, so each issue gets both priority and complexity labels.

**Architecture:** Extend `TriagePanel.svelte` with a two-step flow. After the priority step (existing j/k/s), the card stays visible and shows a complexity step (same j/k/s controls). Both steps are independently skippable. Labels are applied fire-and-forget after both steps complete.

**Tech Stack:** Svelte 5 (runes), Tauri invoke for GitHub label API

---

## Design Reference

### Flow

Each issue card goes through two steps:

**Step 1: Priority** — `j`/left: low, `k`/right: high, `s`/down: skip
**Step 2: Complexity** — `j`/left: simple, `k`/right: complex, `s`/down: skip

Both steps always happen. Skipping one does not skip the other. After both steps, card animates out.

### GitHub Labels

- `complexity: simple` — color: `89DCEB` (Catppuccin sky), description: "Quick task, suitable for simple agents"
- `complexity: complex` — color: `FAB387` (Catppuccin peach), description: "Multi-step task, needs capable agents"

---

### Task 1: Add step state and complexity tracking to TriagePanel

**Files:**
- Modify: `src/lib/TriagePanel.svelte:16-20` (state declarations)

**Step 1: Add step state and complexity counts**

In `src/lib/TriagePanel.svelte`, update the state block (lines 16-20) from:

```svelte
let issues: GithubIssue[] = $state([]);
let currentIndex = $state(0);
let loading = $state(false);
let error: string | null = $state(null);
let swipeDirection: "left" | "right" | null = $state(null);
let triageCount = $state({ high: 0, low: 0, skipped: 0 });
```

to:

```svelte
let issues: GithubIssue[] = $state([]);
let currentIndex = $state(0);
let loading = $state(false);
let error: string | null = $state(null);
let swipeDirection: "left" | "right" | null = $state(null);
let step: "priority" | "complexity" = $state("priority");
let pendingPriority: "high" | "low" | null = $state(null);
let triageCount = $state({ high: 0, low: 0, skipped: 0, simple: 0, complex: 0 });
```

**Step 2: Verify the app still compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-19-1cb7af && npx svelte-check --threshold error 2>&1 | tail -5`
Expected: No errors (warnings about unused vars are fine for now)

**Step 3: Commit**

```bash
git add src/lib/TriagePanel.svelte
git commit -m "feat(triage): add step state and complexity tracking"
```

---

### Task 2: Refactor assignPriority to handle two-step flow

**Files:**
- Modify: `src/lib/TriagePanel.svelte:73-104` (assignPriority and skip functions)

**Step 1: Replace assignPriority and skip with step-aware logic**

Replace the `assignPriority` function (lines 73-97) and `skip` function (lines 99-104) with:

```svelte
async function assignPriority(priority: "high" | "low") {
  if (!currentIssue || !repoPath) return;

  swipeDirection = priority === "high" ? "right" : "left";

  if (priority === "high") triageCount.high++;
  else triageCount.low++;

  await new Promise(r => setTimeout(r, 300));
  swipeDirection = null;
  pendingPriority = priority;
  step = "complexity";
}

async function assignComplexity(complexity: "simple" | "complex") {
  if (!currentIssue || !repoPath) return;

  const issue = currentIssue;
  const priority = pendingPriority;
  swipeDirection = complexity === "complex" ? "right" : "left";

  if (complexity === "simple") triageCount.simple++;
  else triageCount.complex++;

  await new Promise(r => setTimeout(r, 300));
  swipeDirection = null;
  advanceCard(issue, priority, complexity);
}

function skipPriority() {
  if (!currentIssue) return;
  pendingPriority = null;
  step = "complexity";
}

function skipComplexity() {
  if (!currentIssue) return;

  const issue = currentIssue;
  const priority = pendingPriority;
  advanceCard(issue, priority, null);
}

function advanceCard(issue: GithubIssue, priority: "high" | "low" | null, complexity: "simple" | "complex" | null) {
  if (!priority && !complexity) triageCount.skipped++;

  currentIndex++;
  step = "priority";
  pendingPriority = null;

  if (!repoPath) return;
  const path = repoPath;

  // Fire and forget label assignments
  if (priority) {
    invoke("add_github_label", {
      repoPath: path,
      issueNumber: issue.number,
      label: `priority: ${priority}`,
      description: priority === "high" ? "Important, should be tackled soon" : "Nice to have, can wait",
      color: priority === "high" ? "F38BA8" : "A6E3A1",
    }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
  }

  if (complexity) {
    invoke("add_github_label", {
      repoPath: path,
      issueNumber: issue.number,
      label: `complexity: ${complexity}`,
      description: complexity === "simple" ? "Quick task, suitable for simple agents" : "Multi-step task, needs capable agents",
      color: complexity === "simple" ? "89DCEB" : "FAB387",
    }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
  }
}
```

**Step 2: Verify the app still compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-19-1cb7af && npx svelte-check --threshold error 2>&1 | tail -5`

**Step 3: Commit**

```bash
git add src/lib/TriagePanel.svelte
git commit -m "feat(triage): refactor to two-step priority+complexity flow"
```

---

### Task 3: Update keydown handler to be step-aware

**Files:**
- Modify: `src/lib/TriagePanel.svelte:106-129` (handleKeydown function)

**Step 1: Replace handleKeydown with step-aware version**

Replace the `handleKeydown` function (lines 106-129) with:

```svelte
function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    onClose();
    return;
  }

  if (swipeDirection) return; // animating

  if (step === "priority") {
    if (e.key === "ArrowRight" || e.key === "k") {
      e.preventDefault();
      e.stopPropagation();
      assignPriority("high");
    } else if (e.key === "ArrowLeft" || e.key === "j") {
      e.preventDefault();
      e.stopPropagation();
      assignPriority("low");
    } else if (e.key === "s" || e.key === "ArrowDown") {
      e.preventDefault();
      e.stopPropagation();
      skipPriority();
    }
  } else {
    if (e.key === "ArrowRight" || e.key === "k") {
      e.preventDefault();
      e.stopPropagation();
      assignComplexity("complex");
    } else if (e.key === "ArrowLeft" || e.key === "j") {
      e.preventDefault();
      e.stopPropagation();
      assignComplexity("simple");
    } else if (e.key === "s" || e.key === "ArrowDown") {
      e.preventDefault();
      e.stopPropagation();
      skipComplexity();
    }
  }
}
```

**Step 2: Verify the app still compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-19-1cb7af && npx svelte-check --threshold error 2>&1 | tail -5`

**Step 3: Commit**

```bash
git add src/lib/TriagePanel.svelte
git commit -m "feat(triage): make keydown handler step-aware"
```

---

### Task 4: Update UI to show current step

**Files:**
- Modify: `src/lib/TriagePanel.svelte:139-216` (template)

**Step 1: Update the card area and hotkey bar**

Replace the label hints and hotkey bar to reflect the current step. Replace lines 168-214 (the `{:else}` block containing card-area and hotkey-bar) with:

```svelte
  {:else}
    <div class="step-indicator">
      <span class="step-dot" class:active={step === "priority"}></span>
      <span class="step-dot" class:active={step === "complexity"}></span>
      <span class="step-label">{step === "priority" ? "Step 1: Priority" : "Step 2: Complexity"}</span>
    </div>

    <div class="card-area">
      <div class="label-hint left">
        <span class="label-arrow">&#8592;</span>
        <span class="label-text" class:low={step === "priority"} class:simple={step === "complexity"}>
          {step === "priority" ? "Low" : "Simple"}
        </span>
      </div>

      <div
        class="issue-card"
        class:swipe-left={swipeDirection === "left"}
        class:swipe-right={swipeDirection === "right"}
      >
        <div class="card-number">#{currentIssue.number}</div>
        <div class="card-title">{currentIssue.title}</div>
        {#if currentIssue.body}
          <div class="card-body">{currentIssue.body}</div>
        {/if}
        {#if currentIssue.labels.length > 0}
          <div class="card-labels">
            {#each currentIssue.labels as label}
              <span class="card-label">{label.name}</span>
            {/each}
          </div>
        {/if}
        <div class="card-counter">{remaining} remaining</div>
      </div>

      <div class="label-hint right">
        <span class="label-text" class:high={step === "priority"} class:complex={step === "complexity"}>
          {step === "priority" ? "High" : "Complex"}
        </span>
        <span class="label-arrow">&#8594;</span>
      </div>
    </div>

    <div class="hotkey-bar">
      <div class="hotkey-group">
        <kbd>&#8592;</kbd> / <kbd>j</kbd>
        <span class="hotkey-desc">{step === "priority" ? "Low priority" : "Simple"}</span>
      </div>
      <div class="hotkey-group">
        <kbd>&#8595;</kbd> / <kbd>s</kbd>
        <span class="hotkey-desc">Skip</span>
      </div>
      <div class="hotkey-group">
        <kbd>&#8594;</kbd> / <kbd>k</kbd>
        <span class="hotkey-desc">{step === "priority" ? "High priority" : "Complex"}</span>
      </div>
    </div>
  {/if}
```

**Step 2: Update triage stats to include complexity counts**

Replace the triage-stats div in the header (line 144-148) with:

```svelte
    <div class="triage-stats">
      <span class="stat high">{triageCount.high} high</span>
      <span class="stat low">{triageCount.low} low</span>
      <span class="stat simple">{triageCount.simple} simple</span>
      <span class="stat complex">{triageCount.complex} complex</span>
      <span class="stat skipped">{triageCount.skipped} skipped</span>
    </div>
```

**Step 3: Update the done summary similarly**

Replace the done-summary div (lines 162-166) with:

```svelte
      <div class="done-summary">
        <span class="stat high">{triageCount.high} high</span>
        <span class="stat low">{triageCount.low} low</span>
        <span class="stat simple">{triageCount.simple} simple</span>
        <span class="stat complex">{triageCount.complex} complex</span>
        <span class="stat skipped">{triageCount.skipped} skipped</span>
      </div>
```

**Step 4: Verify the app still compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-19-1cb7af && npx svelte-check --threshold error 2>&1 | tail -5`

**Step 5: Commit**

```bash
git add src/lib/TriagePanel.svelte
git commit -m "feat(triage): update UI for two-step priority+complexity flow"
```

---

### Task 5: Add CSS for new elements

**Files:**
- Modify: `src/lib/TriagePanel.svelte` (style block)

**Step 1: Add styles for step indicator and complexity colors**

Add the following CSS rules inside the `<style>` block, after the `.label-text.low` rule (after line 327):

```css
.label-text.simple {
  color: #89dceb;
}

.label-text.complex {
  color: #fab387;
}

.stat.simple {
  color: #89dceb;
  background: rgba(137, 220, 235, 0.1);
}

.stat.complex {
  color: #fab387;
  background: rgba(250, 179, 135, 0.1);
}

.step-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
}

.step-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #313244;
}

.step-dot.active {
  background: #89b4fa;
}

.step-label {
  font-size: 12px;
  color: #6c7086;
  margin-left: 4px;
}
```

**Step 2: Verify the app still compiles**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-19-1cb7af && npx svelte-check --threshold error 2>&1 | tail -5`

**Step 3: Commit**

```bash
git add src/lib/TriagePanel.svelte
git commit -m "feat(triage): add CSS for complexity step indicator and colors"
```

---

### Task 6: Manual verification

**Step 1: Run the app**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-19-1cb7af && npm run tauri dev`

**Step 2: Verify the flow**

1. Press `t` to open triage panel
2. Confirm step 1 shows "Priority" with left=Low, right=High
3. Press `k` (high priority) — card should animate right, then stay showing step 2
4. Confirm step 2 shows "Complexity" with left=Simple, right=Complex
5. Press `j` (simple) — card should animate left, next issue appears at step 1
6. Test skip: press `s` at priority step — should advance to complexity step
7. Test skip: press `s` at complexity step — should advance to next issue
8. When all issues are done, verify the summary shows all 5 counts (high/low/simple/complex/skipped)

**Step 3: Commit (if any fixes needed)**
