import type { WorkspaceMode } from "./stores";

export type CommandSection = "Navigation" | "Panels" | "Agents";

// IDs for commands handled in handleHotkey's switch
export type CommandId =
  | "navigate-next"
  | "navigate-prev"
  | "fuzzy-finder"
  | "expand-collapse"
  | "toggle-agent"
  | "trigger-agent-check"
  | "toggle-help"
  | "clear-agent-reports"
  | "toggle-maintainer-view";

// IDs for commands handled outside handleHotkey (Cmd+key, Escape)
export type ExternalCommandId =
  | "keystroke-visualizer"
  | "switch-workspace"
  | "escape-focus";

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
  { id: "navigate-next", key: "j", section: "Navigation", description: "Next / previous item", helpKey: "j / k" },
  { id: "navigate-prev", key: "k", section: "Navigation", description: "Next / previous item", hidden: true },
  { id: "expand-collapse", key: "l", section: "Navigation", description: "Expand/collapse project or open focused item", helpKey: "l / Enter" },
  { id: "expand-collapse", key: "Enter", section: "Navigation", description: "Expand/collapse project or open focused item", hidden: true },
  { id: "fuzzy-finder", key: "f", section: "Navigation", description: "Find project (fuzzy finder)" },
  { id: "escape-focus", key: "Esc", section: "Navigation", description: "Move focus up", handledExternally: true },

  // ── Panels ──
  { id: "toggle-help", key: "?", section: "Panels", description: "Toggle this help" },
  { id: "switch-workspace", key: "␣", section: "Panels", description: "Switch workspace mode", handledExternally: true, hidden: true },
  { id: "keystroke-visualizer", key: "⌘k", section: "Panels", description: "Toggle keystroke visualizer", handledExternally: true },

  // ── Agents ──
  { id: "toggle-agent", key: "o", section: "Agents", description: "Toggle focused agent on/off", mode: "agents" },
  { id: "trigger-agent-check", key: "r", section: "Agents", description: "Run maintainer check for focused project", mode: "agents" },
  { id: "clear-agent-reports", key: "c", section: "Agents", description: "Clear maintainer reports for focused project", mode: "agents" },
  { id: "toggle-maintainer-view", key: "t", section: "Agents", description: "Toggle between Runs / Issues view", mode: "agents" },

];

// Section order for help display
const SECTION_ORDER: CommandSection[] = ["Navigation", "Panels", "Agents"];

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
