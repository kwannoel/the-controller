# UI Consistency Fixes

**Date:** 2026-03-15
**Status:** Approved

## Context

Visual audit of all workspace modes revealed six design inconsistencies. Screenshots captured via Playwright across Development, Agents, Architecture, Notes, Infrastructure, and Voice modes.

## Fixes

### 1. Sidebar header — show all mode names

**File:** `src/lib/Sidebar.svelte:611`

Replace ternary chain with a map covering all modes:
- `development` → "Development"
- `agents` → "Agents"
- `architecture` → "Architecture"
- `notes` → "Notes"
- `infrastructure` → "Infrastructure"
- `voice` → "Voice"

### 2. Empty state text — standardize pattern

All empty states use two lines, sentence-case title, lowercase hint:

| Mode | Title | Hint |
|---|---|---|
| Development | No active session | press `c` to create a session, or `n` to add a project |
| Agents | No agent selected | navigate to an agent with `j` / `k` and press `l` |
| Notes | No note selected | press `n` to create one |
| Architecture | No architecture generated yet | press `r` to generate |
| Infrastructure | No services deployed yet | press `d` to deploy a project |

Infrastructure changes from three-line (title/subtitle/hint) to standard two-line (title/hint). Uses `.empty-title` / `.empty-hint` class names and same font sizes as other modes (16px/500 title, 13px secondary hint).

### 3. kbd styling — unify inline style, fix Infrastructure

Two intentional kbd styles:
- **Inline kbd** (empty states, hints): `--bg-hover` bg, `--text-emphasis` text, `--font-mono`, 12px
- **Action kbd** (workspace picker, help modal): `--text-emphasis` bg, `--bg-void` text, `--font-mono`, 13px

Fix Infrastructure: change from `--bg-active` bg + `monospace` to standard inline kbd style. Change hint text from `--text-tertiary` to `--text-secondary`.

### 4. Footer spacer — remove dead space

**File:** `src/lib/Sidebar.svelte:669`

Remove `.footer-spacer` div and its CSS rule. Provider indicator sits directly in the footer.

### 5. Help button focus ring — use app convention

**File:** `src/lib/Sidebar.svelte` (style block)

Add to `.btn-help`:
```css
outline: none;
```

Add `.btn-help:focus-visible`:
```css
outline: 2px solid var(--focus-ring);
outline-offset: -2px;
```
