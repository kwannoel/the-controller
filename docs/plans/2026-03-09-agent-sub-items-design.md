# Agent Sub-Items in Agent Workspace

## Goal

Show auto-worker and maintainer as expandable sub-items under each project in the agent workspace sidebar, matching the dev workspace's project → session hierarchy. Each agent has its own focused page accessible via `l`, with `Esc` returning to sidebar.

## Architecture

### New FocusTarget variant

Add to `FocusTarget` union in `stores.ts`:

```typescript
| { type: "agent"; agentKind: "auto-worker" | "maintainer"; projectId: string }
```

### AgentTree sidebar changes

Transform from flat project cards to expandable tree:
- Projects show ▶/▼ expand button, project name, and agent count badge
- When expanded, two sub-items: "Auto-worker" and "Maintainer"
- Each sub-item shows: status dot, label, ON/OFF badge
- Uses `data-agent-id="<projectId>:<agentKind>"` for DOM focus queries
- Reuses `expandedProjects` store (shared with dev workspace)

### Navigation (HotkeyManager)

- `j`/`k` in agents mode traverses projects + agent sub-items (when expanded)
- `J`/`K` skips agents, navigates projects only (already works)
- `l` on project: expand/collapse (same as dev workspace)
- `l` on agent sub-item: set `focusTarget` to `{ type: "agent", agentKind, projectId }` — dashboard reacts
- `Esc` from agent focus: walk up to parent project
- Sidebar stays visible when agent page is focused

### AgentDashboard changes

Reacts to `focusTarget`:
- When `type === "agent"` with `agentKind === "auto-worker"`: show only auto-worker section
- When `type === "agent"` with `agentKind === "maintainer"`: show only maintainer section
- When `type === "project"` or no agent focused: show empty state ("Select an agent with j/k and l")

### No backend changes

All data already available from existing stores (`autoWorkerStatuses`, `maintainerStatuses`, project config).

## Files to modify

1. `src/lib/stores.ts` — Add `"agent"` to `FocusTarget` union
2. `src/lib/sidebar/AgentTree.svelte` — Rewrite as expandable tree with agent sub-items
3. `src/lib/HotkeyManager.svelte` — Agent-mode-aware `getVisibleItems()`, escape handling for agent focus
4. `src/lib/AgentDashboard.svelte` — Scope content to focused agent kind
5. `src/lib/Sidebar.svelte` — Pass additional props to AgentTree (expandedProjectSet, onToggleProject, onAgentFocus)
6. `src/lib/commands.ts` — Update `expand-collapse` description if needed
7. `src/lib/focus-helpers.ts` — Add agent-aware focus helpers if needed
