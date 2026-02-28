# Fuzzy Finder & Streamlined Project UX — Design Document

Replace manual path typing with a configured projects root, fuzzy finder modal, and LLM-generated project names. Add an onboarding flow for first launch.

## Changes Overview

1. **Onboarding** — first-launch setup: projects root directory + Claude CLI auth check
2. **Fuzzy finder** — Cmd+P-style modal to browse projects root children
3. **New project** — describe → LLM suggests 3 names → pick one → auto-scaffolded
4. **Load existing** — fuzzy finder over root dir → select → registered
5. **Session auto-naming** — no label prompt, auto-increment per project
6. **Future:** vim-style keyboard navigation (not this iteration, but all UI must be keyboard-navigable)

## Onboarding Flow

On first launch, check for `~/.the-controller/config.json`. If missing, show onboarding instead of the main layout.

**Step 1: Projects Root Directory**
- Centered card: "Where do your projects live?"
- Text input with placeholder (`~/projects`)
- "Browse" button using Tauri native directory picker
- "Next" validates directory exists

**Step 2: Claude CLI Check**
- Run `which claude` / `claude --version` to check installation
- Run `claude --print "say ok"` to check auth
- Three states:
  - Authenticated: green check, "Claude CLI is ready", continue
  - Not authenticated: "Run `claude login` in your terminal" + "Check Again" button
  - Not installed: "Install Claude CLI from..." + "Check Again" button

**Step 3: Save & Continue**
- Write `~/.the-controller/config.json`: `{ "projects_root": "/Users/noel/projects" }`
- Transition to main app

## Fuzzy Finder Modal

Overlay triggered from sidebar actions. Backend provides directory list, frontend does fuzzy matching.

```
┌─────────────────────────────────────────┐
│  Search projects...                     │
├─────────────────────────────────────────┤
│  ▸ my-api              ~/projects/      │
│  ▸ dashboard           ~/projects/      │
│  ▸ the-controller      ~/projects/      │
│  ▸ website             ~/projects/      │
│                                         │
│  (↑↓ navigate, Enter select, Esc close) │
└─────────────────────────────────────────┘
```

- `list_root_directories()` backend command reads immediate children of `projects_root`
- Frontend caches list, fuzzy filters in JS on each keystroke
- Keyboard-driven: arrow keys, Enter, Esc
- Used for "Load Existing" project flow

## New Project Flow

1. User clicks "+ New" → "Create New"
2. Modal: "Describe your project in a few words" + text input
3. "Generate Names" → backend shells out to `claude --print` with prompt
4. Frontend shows 3 suggestions as clickable/selectable options
5. User picks one (or types custom name)
6. Backend creates `{projects_root}/{name}/`, runs `git init`, registers as project
7. Project appears in sidebar

## Load Existing Flow

1. User clicks "+ New" → "Load Existing"
2. Fuzzy finder modal opens with root dir children
3. User searches, selects a directory
4. Backend registers it as a project (uses directory name as project name)
5. Respects existing `agents.md` if present

## Session Auto-Naming

- Regular sessions: auto-named `session-1`, `session-2`, etc. (incrementing per project)
- Refinement sessions: use branch name as label (unchanged)
- No prompt on session creation — click "+" and it spawns immediately

## Backend Changes

**New commands:**
- `check_onboarding()` → `Result<Option<Config>, String>` — returns config if exists, None if onboarding needed
- `save_config(projects_root: String)` → `Result<(), String>` — validates dir, writes config.json
- `check_claude_cli()` → `Result<String, String>` — returns "authenticated", "not_authenticated", or "not_installed"
- `list_root_directories()` → `Result<Vec<DirEntry>, String>` — immediate children of projects_root
- `generate_project_names(description: String)` → `Result<Vec<String>, String>` — shells out to claude CLI
- `scaffold_project(name: String)` → `Result<Project, String>` — mkdir + git init + register project

**Modified commands:**
- `create_project` — derives `repo_path` from `config.projects_root + name` instead of taking it as param
- `create_session` — auto-generates label (`session-N`), no label param from frontend

**New config file:**
```json
// ~/.the-controller/config.json
{ "projects_root": "/Users/noel/projects" }
```

**New data types:**
```rust
struct Config {
    projects_root: String,
}

struct DirEntry {
    name: String,
    path: String,
}
```

## Frontend Changes

**New components:**
- `Onboarding.svelte` — two-step setup wizard
- `FuzzyFinder.svelte` — modal overlay with search input + filtered list
- `NewProjectModal.svelte` — description input → name suggestions → selection

**Modified components:**
- `App.svelte` — conditionally render Onboarding or main layout based on config state
- `Sidebar.svelte` — replace form with modal triggers, remove label prompt from session creation
