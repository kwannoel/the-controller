import type { WorkspaceMode } from "./stores";

export type CommandSection = "Essentials" | "Navigation" | "Sessions" | "Projects" | "Panels" | "Agents" | "Notes" | "Infrastructure" | "Debug";

// IDs for commands handled in handleHotkey's switch
export type CommandId =
  | "navigate-next"
  | "navigate-prev"
  | "fuzzy-finder"
  | "new-project"
  | "delete"
  | "create-session"
  | "finish-branch"
  | "create-issue"
  | "triage-untriaged"
  | "triage-triaged"
  | "assigned-issues"
  | "expand-collapse"
  | "toggle-agent"
  | "trigger-agent-check"
  | "toggle-help"
  | "clear-agent-reports"
  | "create-note"
  | "delete-note"
  | "rename-note"
  | "duplicate-note"
  | "toggle-note-preview"
  | "save-prompt"
  | "load-prompt"
  | "generate-architecture"
  | "stage"
  | "toggle-maintainer-view"
  | "deploy-project"
  | "rollback-deploy";

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
  { id: "stage", key: "v", section: "Sessions", description: "Stage/unstage session as separate instance", mode: "development" },
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
  { id: "generate-architecture", key: "r", section: "Projects", description: "Generate / regenerate architecture for focused project", mode: "architecture" },

  // ── Panels ──
  { id: "toggle-help", key: "?", section: "Panels", description: "Toggle this help" },
  { id: "switch-workspace", key: "␣", section: "Panels", description: "Switch workspace mode", handledExternally: true, hidden: true },
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
  { id: "duplicate-note", key: "y", section: "Notes", description: "Duplicate focused note", mode: "notes" },
  { id: "toggle-note-preview", key: "p", section: "Notes", description: "Cycle edit / preview / split", mode: "notes" },
  { id: "expand-collapse", key: "o", section: "Notes", description: "Open note for editing", mode: "notes", hidden: true },
  { id: "expand-collapse", key: "i", section: "Notes", description: "Open note for editing", mode: "notes", hidden: true },
  { id: "expand-collapse", key: "a", section: "Notes", description: "Open note for editing", mode: "notes", hidden: true },

  // ── Infrastructure ──
  { id: "deploy-project", key: "d", section: "Infrastructure", description: "Deploy focused project", mode: "infrastructure" },
  { id: "rollback-deploy", key: "r", section: "Infrastructure", description: "Rollback last deployment", mode: "infrastructure" },
];

// Section order for help display
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels", "Agents", "Notes", "Infrastructure"];
const DEV_SECTION_ORDER: CommandSection[] = ["Essentials", "Debug", "Sessions", "Projects", "Panels"];

export interface HelpEntry {
  key: string;
  description: string;
}

export interface HelpSection {
  label: string;
  entries: HelpEntry[];
}

export function getHelpSections(mode?: WorkspaceMode): HelpSection[] {
  if (mode === "development") {
    const essentialIds = new Set(["create-session", "navigate-next", "navigate-prev", "finish-branch", "new-project", "delete", "fuzzy-finder", "expand-collapse", "escape-focus", "escape-forward"]);
    const debugIds = new Set(["screenshot", "screenshot-cropped", "screenshot-preview"]);

    const essentials: HelpSection = {
      label: "Essentials",
      entries: [
        { key: "c", description: "Create session with issue" },
        { key: "j / k", description: "Next / previous item" },
        { key: "n", description: "New project" },
        { key: "d", description: "Delete focused item" },
        { key: "m", description: "Merge session branch" },
        { key: "f", description: "Find project (fuzzy finder)" },
        { key: "l / Enter", description: "Expand/collapse or focus terminal" },
        { key: "Esc", description: "Move focus up" },
        { key: "Esc Esc", description: "Forward escape to terminal" },
      ],
    };

    const debug: HelpSection = {
      label: "Debug",
      entries: commands
        .filter(c => debugIds.has(c.id) && !c.hidden)
        .map(c => ({ key: c.helpKey ?? c.key, description: c.description })),
    };

    const builtSections: Record<string, HelpSection> = { Essentials: essentials, Debug: debug };

    const result = DEV_SECTION_ORDER.map(sectionName => {
      if (builtSections[sectionName]) return builtSections[sectionName];
      return {
        label: sectionName,
        entries: commands
          .filter(c => c.section === sectionName && !c.hidden)
          .filter(c => !c.mode || c.mode === mode)
          .filter(c => !essentialIds.has(c.id) && !debugIds.has(c.id))
          .map(c => ({ key: c.helpKey ?? c.key, description: c.description })),
      };
    }).filter(s => s.entries.length > 0);

    return result;
  }

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
