export type CommandSection = "Navigation" | "Sessions" | "Projects" | "Panels";

// IDs for commands handled in handleHotkey's switch
export type CommandId =
  | "navigate-next"
  | "navigate-prev"
  | "navigate-project-next"
  | "navigate-project-prev"
  | "jump-mode"
  | "fuzzy-finder"
  | "new-project"
  | "delete"
  | "archive"
  | "toggle-archive-view"
  | "create-session-claude"
  | "create-session-codex"
  | "background-worker-claude"
  | "background-worker-codex"
  | "finish-branch"
  | "toggle-sidebar"
  | "create-issue"
  | "triage-untriaged"
  | "triage-triaged"
  | "expand-collapse"
  | "toggle-mode"
  | "trigger-maintainer-check"
  | "toggle-maintainer-panel"
  | "toggle-help";

// IDs for commands handled outside handleHotkey (Cmd+key, Escape)
export type ExternalCommandId =
  | "screenshot"
  | "screenshot-cropped"
  | "screenshot-preview"
  | "keystroke-visualizer"
  | "escape-focus"
  | "escape-forward"
  | "clear-maintainer-reports";

export interface CommandDef {
  id: CommandId | ExternalCommandId;
  key: string;
  section: CommandSection;
  description: string;
  helpKey?: string;       // Display override for help (e.g., "j / k")
  hidden?: boolean;       // Don't show in help (paired secondary keys)
  handledExternally?: boolean;  // Handled in onKeydown, not handleHotkey
}

export const commands: CommandDef[] = [
  // ── Navigation ──
  { id: "navigate-next", key: "j", section: "Navigation", description: "Next / previous item (project or session)", helpKey: "j / k" },
  { id: "navigate-prev", key: "k", section: "Navigation", description: "Next / previous item (project or session)", hidden: true },
  { id: "navigate-project-next", key: "J", section: "Navigation", description: "Next / previous project (skip sessions)", helpKey: "J / K" },
  { id: "navigate-project-prev", key: "K", section: "Navigation", description: "Next / previous project (skip sessions)", hidden: true },
  { id: "expand-collapse", key: "l", section: "Navigation", description: "Expand/collapse project or focus terminal", helpKey: "l / Enter" },
  { id: "expand-collapse", key: "Enter", section: "Navigation", description: "Expand/collapse project or focus terminal", hidden: true },
  { id: "jump-mode", key: "g", section: "Navigation", description: "Go to project / session (jump mode)" },
  { id: "fuzzy-finder", key: "f", section: "Navigation", description: "Find project (fuzzy finder)" },
  { id: "escape-focus", key: "Esc", section: "Navigation", description: "Move focus up (terminal → session → project)", handledExternally: true },
  { id: "escape-forward", key: "Esc Esc", section: "Navigation", description: "Forward escape to terminal", handledExternally: true },

  // ── Sessions ──
  { id: "create-session-claude", key: "c", section: "Sessions", description: "Create Claude session with issue" },
  { id: "create-session-codex", key: "x", section: "Sessions", description: "Create Codex session with issue" },
  { id: "background-worker-claude", key: "C", section: "Sessions", description: "Background worker: Claude (autonomous)" },
  { id: "background-worker-codex", key: "X", section: "Sessions", description: "Background worker: Codex (autonomous)" },
  { id: "finish-branch", key: "m", section: "Sessions", description: "Merge session branch (create PR)" },
  { id: "screenshot", key: "⌘S", section: "Sessions", description: "Screenshot (full) → new session", handledExternally: true },
  { id: "screenshot-cropped", key: "⌘D", section: "Sessions", description: "Screenshot (cropped) → new session", handledExternally: true },
  { id: "screenshot-preview", key: "⌘⇧S / ⌘⇧D", section: "Sessions", description: "Screenshot with preview before sending", handledExternally: true },

  // ── Projects ──
  { id: "new-project", key: "n", section: "Projects", description: "New project" },
  { id: "delete", key: "d", section: "Projects", description: "Delete focused item (session or project)" },
  { id: "archive", key: "a", section: "Projects", description: "Archive focused item (session or project)" },
  { id: "toggle-archive-view", key: "A", section: "Projects", description: "View archived projects" },
  { id: "create-issue", key: "i", section: "Projects", description: "Create GitHub issue for focused project" },
  { id: "triage-untriaged", key: "t", section: "Projects", description: "Triage issues (untriaged)" },
  { id: "triage-triaged", key: "T", section: "Projects", description: "View triaged issues" },

  // ── Panels ──
  { id: "toggle-sidebar", key: "s", section: "Panels", description: "Toggle sidebar" },
  { id: "toggle-maintainer-panel", key: "b", section: "Panels", description: "Toggle background agent panel" },
  { id: "toggle-mode", key: "o", section: "Panels", description: "Toggle: (m)aintainer / (w)orker" },
  { id: "trigger-maintainer-check", key: "r", section: "Panels", description: "Run maintainer check now (when panel open)" },
  { id: "clear-maintainer-reports", key: "c", section: "Panels", description: "Clear maintainer reports (when panel open)", handledExternally: true },
  { id: "toggle-help", key: "?", section: "Panels", description: "Toggle this help" },
  { id: "keystroke-visualizer", key: "⌘K", section: "Panels", description: "Toggle keystroke visualizer", handledExternally: true },
];

// Section order for help display
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels"];

export interface HelpEntry {
  key: string;
  description: string;
}

export interface HelpSection {
  label: string;
  entries: HelpEntry[];
}

export function getHelpSections(): HelpSection[] {
  return SECTION_ORDER.map(section => ({
    label: section,
    entries: commands
      .filter(c => c.section === section && !c.hidden)
      .map(c => ({ key: c.helpKey ?? c.key, description: c.description })),
  }));
}

// Build key→CommandId map for handleHotkey (excludes external commands)
export function buildKeyMap(): Map<string, CommandId> {
  const map = new Map<string, CommandId>();
  for (const cmd of commands) {
    if (cmd.handledExternally) continue;
    map.set(cmd.key, cmd.id as CommandId);
  }
  return map;
}
