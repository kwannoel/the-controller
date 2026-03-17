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
  | "open-issues-modal"
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
  | "deploy-project";

// IDs for commands handled outside handleHotkey (Cmd+key, Escape)
export type ExternalCommandId =
  | "screenshot"
  | "screenshot-cropped"
  | "screenshot-picker"
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
  { id: "create-session", key: "c", section: "Sessions", description: "Create session", mode: "development" },
  { id: "finish-branch", key: "m", section: "Sessions", description: "Merge session branch (create PR)", mode: "development" },
  { id: "save-prompt", key: "P", section: "Sessions", description: "Save focused session's prompt", mode: "development" },
  { id: "load-prompt", key: "p", section: "Sessions", description: "Load saved prompt into new session", mode: "development" },
  { id: "stage", key: "v", section: "Sessions", description: "Stage/unstage session as separate instance", mode: "development" },
  { id: "screenshot", key: "⌘s", section: "Sessions", description: "Screenshot (full) → new session", handledExternally: true },
  { id: "screenshot-cropped", key: "⌘d", section: "Sessions", description: "Screenshot (cropped) → new session", handledExternally: true },
  { id: "screenshot-picker", key: "⌘S / ⌘D", section: "Sessions", description: "Screenshot → pick session", handledExternally: true },
  { id: "toggle-session-provider", key: "⌘t", section: "Sessions", description: "Cycle session provider", handledExternally: true, mode: "development" },

  // ── Projects ──
  { id: "new-project", key: "n", section: "Projects", description: "New project", mode: "development" },
  { id: "delete", key: "d", section: "Projects", description: "Delete focused item (session or project)", mode: "development" },
  { id: "open-issues-modal", key: "i", section: "Projects", description: "Issues (create, find, assign)", mode: "development" },
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
  { id: "delete-note", key: "d", section: "Notes", description: "Delete focused note or folder", mode: "notes" },
  { id: "rename-note", key: "r", section: "Notes", description: "Rename focused note or folder", mode: "notes" },
  { id: "duplicate-note", key: "y", section: "Notes", description: "Duplicate focused note", mode: "notes" },
  { id: "toggle-note-preview", key: "p", section: "Notes", description: "Cycle edit / preview / split", mode: "notes" },
  { id: "expand-collapse", key: "o", section: "Notes", description: "Open note for editing", mode: "notes", hidden: true },
  { id: "expand-collapse", key: "i", section: "Notes", description: "Open note for editing", mode: "notes", hidden: true },
  { id: "expand-collapse", key: "a", section: "Notes", description: "Open note for editing", mode: "notes", hidden: true },

  // ── Infrastructure ──
  { id: "deploy-project", key: "d", section: "Infrastructure", description: "Deploy focused project", mode: "infrastructure" },
];

/**
 * Convert modifier prefixes for display based on the active meta key.
 * - `Meta+x` → `⌘x` (cmd) or `⌃x` (ctrl)
 * - When meta is ctrl, also converts legacy `⌘x` → `⌃x`
 */
export function formatDisplayKey(key: string, meta: "cmd" | "ctrl"): string {
  let result = key.replaceAll("Meta+", meta === "ctrl" ? "⌃" : "⌘");
  if (meta === "ctrl") {
    result = result.replaceAll("⌘", "⌃");
  }
  return result;
}

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

function resolveKey(cmds: CommandDef[], id: string): string {
  // Prefer non-hidden (overridable) entry; fall back to hidden (alias) entry
  return cmds.find(c => c.id === id && !c.hidden)?.key
    ?? cmds.find(c => c.id === id)?.key
    ?? "?";
}

/** Uppercase the last character of a key string to produce its Shift variant. */
function shiftVariant(key: string): string {
  if (key.length === 0) return key;
  return key.slice(0, -1) + key.slice(-1).toUpperCase();
}

/** Build the screenshot-picker display key from the resolved screenshot keys. */
function screenshotPickerKey(cmds: CommandDef[], fmt: (k: string) => string): string {
  return `${fmt(shiftVariant(resolveKey(cmds, "screenshot")))} / ${fmt(shiftVariant(resolveKey(cmds, "screenshot-cropped")))}`;
}

/** Map a command to its help entry, composing paired keys dynamically. */
function mapCmdToEntry(c: CommandDef, cmds: CommandDef[], fmt: (k: string) => string): HelpEntry {
  if (c.id === "navigate-next") {
    return {
      key: `${fmt(resolveKey(cmds, "navigate-next"))} / ${fmt(resolveKey(cmds, "navigate-prev"))}`,
      description: c.description,
    };
  }
  if (c.id === "expand-collapse" && !c.mode) {
    return {
      key: `${fmt(resolveKey(cmds, "expand-collapse"))} / Enter`,
      description: c.description,
    };
  }
  if (c.id === "screenshot-picker") {
    return { key: screenshotPickerKey(cmds, fmt), description: c.description };
  }
  return { key: fmt(c.helpKey ?? c.key), description: c.description };
}

export function getHelpSections(
  mode?: WorkspaceMode,
  resolvedCmds?: CommandDef[],
  metaKeyValue?: "cmd" | "ctrl",
): HelpSection[] {
  const cmds = resolvedCmds ?? commands;
  const fmt = (k: string) => formatDisplayKey(k, metaKeyValue ?? "cmd");

  if (mode === "development") {
    const essentialIds = new Set(["create-session", "navigate-next", "navigate-prev", "finish-branch", "new-project", "delete", "fuzzy-finder", "expand-collapse", "escape-focus", "escape-forward"]);
    const debugIds = new Set(["screenshot", "screenshot-cropped", "screenshot-picker"]);

    const essentials: HelpSection = {
      label: "Essentials",
      entries: [
        { key: fmt(resolveKey(cmds, "create-session")), description: "Create session" },
        { key: `${fmt(resolveKey(cmds, "navigate-next"))} / ${fmt(resolveKey(cmds, "navigate-prev"))}`, description: "Next / previous item" },
        { key: fmt(resolveKey(cmds, "new-project")), description: "New project" },
        { key: fmt(resolveKey(cmds, "delete")), description: "Delete focused item" },
        { key: fmt(resolveKey(cmds, "finish-branch")), description: "Merge session branch" },
        { key: fmt(resolveKey(cmds, "fuzzy-finder")), description: "Find project (fuzzy finder)" },
        { key: `${fmt(resolveKey(cmds, "expand-collapse"))} / Enter`, description: "Expand/collapse or focus terminal" },
        { key: "Esc", description: "Move focus up" },
        { key: "Esc Esc", description: "Forward escape to terminal" },
      ],
    };

    const debug: HelpSection = {
      label: "Debug",
      entries: cmds
        .filter(c => debugIds.has(c.id) && !c.hidden)
        .map(c => mapCmdToEntry(c, cmds, fmt)),
    };

    const builtSections: Record<string, HelpSection> = { Essentials: essentials, Debug: debug };

    const result = DEV_SECTION_ORDER.map(sectionName => {
      if (builtSections[sectionName]) return builtSections[sectionName];
      return {
        label: sectionName,
        entries: cmds
          .filter(c => c.section === sectionName && !c.hidden)
          .filter(c => !c.mode || c.mode === mode)
          .filter(c => !essentialIds.has(c.id) && !debugIds.has(c.id))
          .map(c => ({ key: fmt(c.helpKey ?? c.key), description: c.description })),
      };
    }).filter(s => s.entries.length > 0);

    return result;
  }

  return SECTION_ORDER.map(section => ({
    label: section,
    entries: cmds
      .filter(c => c.section === section && !c.hidden)
      .filter(c => !c.mode || !mode || c.mode === mode)
      .map(c => mapCmdToEntry(c, cmds, fmt)),
  })).filter(s => s.entries.length > 0);
}

/**
 * Takes the default commands array and a map of command_id→key overrides.
 * Returns a new array with overridden keys applied. Hidden entries that
 * are true aliases (a non-hidden sibling with the same id exists) keep
 * their original keys; hidden entries that are the sole entry for their
 * id are still overridable.
 * If overrides is empty, returns the original array (same reference).
 */
export function applyOverrides(
  cmds: CommandDef[],
  overrides: Record<string, string>,
): CommandDef[] {
  const keys = Object.keys(overrides);
  if (keys.length === 0) return cmds;

  // Check if any override actually matches a known command id
  const ids = new Set(cmds.map(c => c.id));
  const hasMatch = keys.some(k => ids.has(k as CommandId | ExternalCommandId));
  if (!hasMatch) return cmds;

  // IDs that have at least one non-hidden entry are "true aliases" when hidden
  const idsWithNonHidden = new Set(
    cmds.filter(c => !c.hidden).map(c => c.id),
  );

  return cmds.map(cmd => {
    // Skip hidden entries that are true aliases (a non-hidden sibling exists)
    if (cmd.hidden && idsWithNonHidden.has(cmd.id)) return cmd;
    const override = overrides[cmd.id];
    if (override === undefined) return cmd;
    return { ...cmd, key: override, helpKey: undefined };
  });
}

// Build key→CommandId map for handleHotkey (excludes external commands)
export function buildKeyMap(
  mode?: WorkspaceMode,
  resolvedCommands?: CommandDef[],
): Map<string, CommandId> {
  const cmds = resolvedCommands ?? commands;
  const map = new Map<string, CommandId>();
  for (const cmd of cmds) {
    if (cmd.handledExternally) continue;
    if (mode && cmd.mode && cmd.mode !== mode) continue;
    map.set(cmd.key, cmd.id as CommandId);
  }
  return map;
}
