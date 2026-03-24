# Agent Browser OpenClaw Relay Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Replace the old `agent-browser` skill with a relay-first Chromium skill that teaches agents to control an existing logged-in tab before falling back to a managed browser.

**Architecture:** Create a new `skills/agent-browser` directory in this worktree, using the prior skill's shape as the starting point but rewriting the content around OpenClaw's extension-relay model. Keep the command vocabulary familiar, move relay mode to the top of the skill and auth docs, update helper templates to assume an attached browser tab, and align `firefox-browser` with the new comparison story.

**Tech Stack:** Markdown skill docs, shell templates, `rg`, `git`

---

### Task 1: Restore the `agent-browser` skill skeleton in this worktree

**Files:**
- Create: `skills/agent-browser/SKILL.md`
- Create: `skills/agent-browser/references/authentication.md`
- Create: `skills/agent-browser/references/commands.md`
- Create: `skills/agent-browser/references/profiling.md`
- Create: `skills/agent-browser/references/proxy-support.md`
- Create: `skills/agent-browser/references/session-management.md`
- Create: `skills/agent-browser/references/snapshot-refs.md`
- Create: `skills/agent-browser/references/video-recording.md`
- Create: `skills/agent-browser/templates/authenticated-session.sh`
- Create: `skills/agent-browser/templates/capture-workflow.sh`
- Create: `skills/agent-browser/templates/form-automation.sh`

**Step 1: Confirm the skill is missing in this worktree**

Run:

```bash
find skills/agent-browser -maxdepth 3 -type f
```

Expected: command fails with `No such file or directory`.

**Step 2: Copy the previous skill structure into the current worktree**

Run:

```bash
mkdir -p skills/agent-browser/references skills/agent-browser/templates
cp /Users/noel/projects/the-controller/skills/agent-browser/SKILL.md skills/agent-browser/SKILL.md
cp /Users/noel/projects/the-controller/skills/agent-browser/references/*.md skills/agent-browser/references/
cp /Users/noel/projects/the-controller/skills/agent-browser/templates/*.sh skills/agent-browser/templates/
```

Expected: the current worktree now contains a local `skills/agent-browser` tree that can be edited without touching the other checkout.

**Step 3: Verify the copied files exist**

Run:

```bash
find skills/agent-browser -maxdepth 3 -type f | sort
```

Expected: the file list matches the copied skill skeleton.

**Step 4: Commit the restored skeleton**

```bash
git add skills/agent-browser
git commit -m "chore: restore agent-browser skill files in worktree"
```

---

### Task 2: Rewrite `agent-browser` around relay mode

**Files:**
- Modify: `skills/agent-browser/SKILL.md`

**Step 1: Rewrite the frontmatter and overview**

In `skills/agent-browser/SKILL.md`:

- keep the skill name as `agent-browser`
- update the description so it emphasizes controlling an existing Chrome or Chromium tab
- keep the `allowed-tools` entry unless the underlying CLI command changed
- replace the current introduction so the first model is extension relay into the user's existing browser tab

**Step 2: Replace the top-level workflow**

Replace the current launch-first workflow with:

1. start or verify the local relay
2. open the target site in Chrome or Chromium
3. click the extension on the target tab to attach relay control
4. run `agent-browser` commands against that attached tab
5. detach when finished

Include one concrete example sequence in the top section.

**Step 3: Replace the authentication section**

Rewrite the auth section so the order is:

- relay mode with an already logged-in browser tab
- managed dedicated profile fallback
- imported state or saved session as edge cases

Delete wording that treats auth import or saved state as the primary path.

**Step 4: Add an explicit mode table**

Add a short section that defines:

- `relay mode`
- `managed mode`

For each mode, explain when to choose it and what tradeoff it carries.

**Step 5: Verify the new priority order**

Run:

```bash
rg -n "relay mode|managed mode|existing Chrome|existing Chromium|already logged in" skills/agent-browser/SKILL.md
```

Expected: the top-level skill now mentions relay-first concepts before profile or state-file reuse.

**Step 6: Commit**

```bash
git add skills/agent-browser/SKILL.md
git commit -m "docs: rewrite agent-browser skill around relay mode"
```

---

### Task 3: Rewrite the auth and reference docs to match the relay model

**Files:**
- Modify: `skills/agent-browser/references/authentication.md`
- Modify: `skills/agent-browser/references/session-management.md`
- Modify: `skills/agent-browser/references/commands.md`
- Modify: `skills/agent-browser/references/snapshot-refs.md`
- Modify: `skills/agent-browser/references/profiling.md`
- Modify: `skills/agent-browser/references/proxy-support.md`
- Modify: `skills/agent-browser/references/video-recording.md`

**Step 1: Rewrite `references/authentication.md`**

Move the "existing browser" relay flow to the top of the document. Cover:

- installing the extension
- attaching the relay on the right tab
- security note for controlling a real logged-in tab
- how to recover when the wrong tab is attached or relay is not active

Keep managed profile and saved-state guidance as fallback sections lower in the file.

**Step 2: Update session and commands docs**

In `references/session-management.md` and `references/commands.md`, remove wording that assumes the tool launches a fresh browser by default. Replace it with wording that says commands operate on the currently attached tab in relay mode, or on the managed browser in fallback mode.

**Step 3: Update the remaining references**

In `snapshot-refs.md`, `profiling.md`, `proxy-support.md`, and `video-recording.md`, fix any language that conflicts with relay mode or implies Firefox is part of the main path.

**Step 4: Verify stale wording is gone**

Run:

```bash
rg -n "auto-connect state save|Connect to the user's running Chrome|fresh browser|launch a fresh browser" skills/agent-browser/references
```

Expected: no stale primary-flow wording remains, or the remaining matches are clearly labeled as fallback behavior.

**Step 5: Commit**

```bash
git add skills/agent-browser/references
git commit -m "docs: align agent-browser references with relay-first workflow"
```

---

### Task 4: Update helper templates for attached-tab workflows

**Files:**
- Modify: `skills/agent-browser/templates/authenticated-session.sh`
- Modify: `skills/agent-browser/templates/capture-workflow.sh`
- Modify: `skills/agent-browser/templates/form-automation.sh`

**Step 1: Rewrite the authenticated session template**

Change `templates/authenticated-session.sh` so it assumes the user attaches relay control to an existing login tab before the script runs. The script should:

- verify relay access first
- inspect the current page
- save fallback state only if the user asks for it
- stop teaching a login-first state-file workflow as the default

**Step 2: Rewrite the capture template**

Change `templates/capture-workflow.sh` so it can operate against the attached tab without closing the user's browser. Only include close or detach instructions if they are explicitly optional.

**Step 3: Rewrite the form template**

Change `templates/form-automation.sh` so it assumes the current attached tab is already open on the target form, or clearly labels any navigation step as optional.

**Step 4: Verify the templates no longer close the user's browser by default**

Run:

```bash
rg -n "agent-browser close" skills/agent-browser/templates
```

Expected: either no matches remain or any remaining match is in an optional cleanup comment, not the default flow.

**Step 5: Commit**

```bash
git add skills/agent-browser/templates
git commit -m "docs: update agent-browser templates for attached-tab workflows"
```

---

### Task 5: Update `firefox-browser` to reflect the new `agent-browser` model

**Files:**
- Modify: `skills/firefox-browser/SKILL.md`

**Step 1: Rewrite the comparison section**

Update the "Key Differences from agent-browser" table so it compares Firefox Playwright against the new relay-first Chromium model. Make these points explicit:

- `agent-browser` is the preferred skill for reusing an existing logged-in browser
- `firefox-browser` uses Playwright-driven sessions and persistent profiles instead of existing-tab relay control
- `firefox-browser` is not the primary path for "use my current browser"

**Step 2: Verify the comparison text**

Run:

```bash
rg -n "existing logged-in browser|relay|persistent profile|current browser" skills/firefox-browser/SKILL.md
```

Expected: the Firefox skill now points users to `agent-browser` for existing-browser reuse.

**Step 3: Commit**

```bash
git add skills/firefox-browser/SKILL.md
git commit -m "docs: align firefox-browser with relay-first agent-browser"
```

---

### Task 6: Final verification

**Files:**
- Modify: none

**Step 1: Check for formatting errors**

Run:

```bash
git diff --check HEAD~5..HEAD
```

Expected: no output.

**Step 2: Verify the new skill tree and relay wording**

Run:

```bash
find skills/agent-browser -maxdepth 3 -type f | sort
rg -n "relay mode|existing Chrome|existing Chromium|extension|attach" skills/agent-browser
```

Expected: the new skill files exist and the relay workflow appears across the main skill and references.

**Step 3: Verify no contradictory Firefox messaging remains**

Run:

```bash
rg -n "Key Differences from agent-browser|agent-browser" skills/firefox-browser/SKILL.md
```

Expected: the Firefox skill comparison now matches the relay-first Chromium story.

**Step 4: Commit the verification checkpoint**

```bash
git commit --allow-empty -m "chore: verify relay-first browser skill docs"
```
