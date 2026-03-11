import type { WorkspaceMode } from "./stores";

export type CommandSection = "Navigation" | "Sessions" | "Projects" | "Panels" | "Agents" | "Notes";

// IDs for commands handled in handleHotkey's switch
export type CommandId =
  | "navigate-next"
  | "navigate-prev"
  | "navigate-project-next"
  | "navigate-project-prev"
  | "fuzzy-finder"
  | "new-project"
  | "delete"
  | "create-session"
  | "finish-branch"
  | "toggle-sidebar"
  | "create-issue"
  | "triage-untriaged"
  | "triage-triaged"
  | "assigned-issues"
  | "expand-collapse"
  | "toggle-mode"
  | "toggle-agent"
  | "trigger-agent-check"
  | "toggle-help"
  | "clear-agent-reports"
  | "create-note"
  | "delete-note"
  | "rename-note"
  | "toggle-note-preview"
  | "save-prompt"
  | "load-prompt"
  | "stage-inplace"
  | "toggle-maintainer-view"
  | "toggle-controller-chat";

// IDs for commands handled outside handleHotkey (Cmd+key, Escape)
export type ExternalCommandId =
  | "screenshot"
  | "screenshot-cropped"
  | "screenshot-preview"
  | "toggle-session-provider"
  | "keystroke-visualizer"
  | "switch-workspace"
  | "escape-focus"
  | "escape-forward";

export interface CommandDef {
  id: CommandId | ExternalCommandId;
  key: string;
  section: CommandSection;
  description: string;
  helpKey?: string;       // Display override for help (e.g., "j / k")
  hidden?: boolean;       // Don't show in help (paired secondary keys)
  handledExternally?: boolean;  // Handled in onKeydown, not handleHotkey
  mode?: WorkspaceMode;  // undefined = global (available in all modes)
}

export const commands: CommandDef[] = [
  // ── Navigation ──
  { id: "navigate-next", key: "j", section: "Navigation", description: "Next / previous item (project or session)", helpKey: "j / k" },
  { id: "navigate-prev", key: "k", section: "Navigation", description: "Next / previous item (project or session)", hidden: true },
  { id: "navigate-project-next", key: "J", section: "Navigation", description: "Next / previous project (skip sessions)", helpKey: "J / K" },
  { id: "navigate-project-prev", key: "K", section: "Navigation", description: "Next / previous project (skip sessions)", hidden: true },
  { id: "expand-collapse", key: "l", section: "Navigation", description: "Expand/collapse project or focus terminal", helpKey: "l / Enter" },
  { id: "expand-collapse", key: "Enter", section: "Navigation", description: "Expand/collapse project or focus terminal", hidden: true },
  { id: "fuzzy-finder", key: "f", section: "Navigation", description: "Find project (fuzzy finder)" },
  { id: "escape-focus", key: "Esc", section: "Navigation", description: "Move focus up (terminal → session → project)", handledExternally: true },
  { id: "escape-forward", key: "Esc Esc", section: "Navigation", description: "Forward escape to terminal", handledExternally: true },

  // ── Sessions ──
  { id: "create-session", key: "c", section: "Sessions", description: "Create session with issue", mode: "development" },
  { id: "finish-branch", key: "m", section: "Sessions", description: "Merge session branch (create PR)", mode: "development" },
  { id: "save-prompt", key: "P", section: "Sessions", description: "Save focused session's prompt", mode: "development" },
  { id: "load-prompt", key: "p", section: "Sessions", description: "Load saved prompt into new session", mode: "development" },
  { id: "stage-inplace", key: "v", section: "Sessions", description: "Stage/unstage session branch in main repo", mode: "development" },
  { id: "screenshot", key: "⌘s", section: "Sessions", description: "Screenshot (full) → new session", handledExternally: true },
  { id: "screenshot-cropped", key: "⌘d", section: "Sessions", description: "Screenshot (cropped) → new session", handledExternally: true },
  { id: "screenshot-preview", key: "⌘S / ⌘D", section: "Sessions", description: "Screenshot with preview before sending", handledExternally: true },
  { id: "toggle-session-provider", key: "⌘t", section: "Sessions", description: "Toggle session provider", handledExternally: true, mode: "development" },

  // ── Projects ──
  { id: "new-project", key: "n", section: "Projects", description: "New project", mode: "development" },
  { id: "delete", key: "d", section: "Projects", description: "Delete focused item (session or project)", mode: "development" },
  { id: "create-issue", key: "i", section: "Projects", description: "Create GitHub issue for focused project", mode: "development" },
  { id: "triage-untriaged", key: "t", section: "Projects", description: "Triage issues (untriaged)", mode: "development" },
  { id: "triage-triaged", key: "T", section: "Projects", description: "View triaged issues", mode: "development" },
  { id: "assigned-issues", key: "e", section: "Projects", description: "View assigned but uncompleted issues", mode: "development" },

  // ── Panels ──
  { id: "toggle-sidebar", key: "s", section: "Panels", description: "Toggle sidebar" },
  { id: "toggle-controller-chat", key: "g", section: "Panels", description: "Toggle controller chat" },
  { id: "toggle-mode", key: "o", section: "Panels", description: "Toggle: (m)aintainer / (w)orker", mode: "development" },
  { id: "toggle-help", key: "?", section: "Panels", description: "Toggle this help" },
  { id: "switch-workspace", key: "␣", section: "Panels", description: "Switch workspace mode", handledExternally: true },
  { id: "keystroke-visualizer", key: "⌘k", section: "Panels", description: "Toggle keystroke visualizer", handledExternally: true },

  // ── Agents ──
  { id: "toggle-agent", key: "o", section: "Agents", description: "Toggle focused agent on/off", mode: "agents" },
  { id: "trigger-agent-check", key: "r", section: "Agents", description: "Run maintainer check for focused project", mode: "agents" },
  { id: "clear-agent-reports", key: "c", section: "Agents", description: "Clear maintainer reports for focused project", mode: "agents" },
  { id: "toggle-maintainer-view", key: "t", section: "Agents", description: "Toggle between Runs / Issues view", mode: "agents" },

  // ── Notes ──
  { id: "create-note", key: "n", section: "Notes", description: "Create new note", mode: "notes" },
  { id: "delete-note", key: "d", section: "Notes", description: "Delete focused note", mode: "notes" },
  { id: "rename-note", key: "r", section: "Notes", description: "Rename focused note", mode: "notes" },
  { id: "toggle-note-preview", key: "p", section: "Notes", description: "Cycle edit / preview / split", mode: "notes" },
];

// Section order for help display
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels", "Agents", "Notes"];

export interface HelpEntry {
  key: string;
  description: string;
}

export interface HelpSection {
  label: string;
  entries: HelpEntry[];
}

export function getHelpSections(mode?: WorkspaceMode): HelpSection[] {
  return SECTION_ORDER.map(section => ({
    label: section,
    entries: commands
      .filter(c => c.section === section && !c.hidden)
      .filter(c => !c.mode || !mode || c.mode === mode)
      .map(c => ({ key: c.helpKey ?? c.key, description: c.description })),
  })).filter(s => s.entries.length > 0);
}

// Build key→CommandId map for handleHotkey (excludes external commands)
export function buildKeyMap(mode?: WorkspaceMode): Map<string, CommandId> {
  const map = new Map<string, CommandId>();
  for (const cmd of commands) {
    if (cmd.handledExternally) continue;
    if (mode && cmd.mode && cmd.mode !== mode) continue;
    map.set(cmd.key, cmd.id as CommandId);
  }
  return map;
}
