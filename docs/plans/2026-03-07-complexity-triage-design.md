# Combined Priority + Complexity Triage

## Problem

Issues need to be triaged by complexity (simple/complex) in addition to priority (high/low) so agents can be assigned appropriately.

## Design

Extend the existing triage card flow with a second step. After choosing priority, the same card stays visible and prompts for complexity before advancing.

### Flow

Each issue card goes through two steps:

**Step 1: Priority**
- `j` / left arrow: low priority
- `k` / right arrow: high priority
- `s` / down arrow: skip (no priority label)

**Step 2: Complexity**
- `j` / left arrow: simple
- `k` / right arrow: complex
- `s` / down arrow: skip (no complexity label)

Both steps always happen. Skipping one does not skip the other — each dimension is independent. After both steps complete, the card animates out and the next issue appears.

### GitHub Labels

- `complexity: simple` — color: `#89dceb` (Catppuccin sky), description: "Quick task, suitable for simple agents"
- `complexity: complex` — color: `#fab387` (Catppuccin peach), description: "Multi-step task, needs capable agents"

### UI Changes

- **TriagePanel.svelte**: Add a `step` state (`"priority"` | `"complexity"`). After priority is chosen, transition to complexity step. Card header/prompt updates to show current step.
- **Triage stats**: Show simple/complex counts alongside high/low/skipped.

### What Stays the Same

- Hotkeys `t`/`T`, category picker, filtering logic — unchanged
- Escape closes the panel at any point
- Swipe animation direction: left for lesser (low/simple), right for greater (high/complex)
