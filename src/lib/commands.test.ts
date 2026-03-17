import { describe, it, expect } from "vitest";
import { applyOverrides, commands, getHelpSections, buildKeyMap, formatDisplayKey } from "./commands";

describe("command registry", () => {
  it("every non-external command has a unique key within its mode", () => {
    const internal = commands.filter(c => !c.handledExternally);
    const globalKeys = internal.filter(c => !c.mode).map(c => c.key);
    const globalSet = new Set(globalKeys);
    expect(globalKeys.length).toBe(globalSet.size);

    const modes = ["development", "agents", "notes", "architecture", "infrastructure"] as const;
    for (const mode of modes) {
      const modeKeys = internal.filter(c => c.mode === mode).map(c => c.key);
      const allKeys = [...globalKeys, ...modeKeys];
      const allSet = new Set(allKeys);
      expect(allKeys.length).toBe(allSet.size);
    }
  });

  it("every non-hidden command has a description", () => {
    for (const cmd of commands.filter(c => !c.hidden)) {
      expect(cmd.description.length).toBeGreaterThan(0);
    }
  });

  it("getHelpSections returns sections in order for development mode", () => {
    const sections = getHelpSections("development");
    expect(sections.map(s => s.label)).toEqual(["Essentials", "Debug", "Sessions", "Projects", "Panels"]);
  });

  it("getHelpSections returns sections for agents mode", () => {
    const sections = getHelpSections("agents");
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Panels", "Agents"]);
  });

  it("getHelpSections returns sections for notes mode", () => {
    const sections = getHelpSections("notes");
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Panels", "Notes"]);
  });

  it("getHelpSections without mode returns all sections", () => {
    const sections = getHelpSections();
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Projects", "Panels", "Agents", "Notes", "Infrastructure"]);
  });

  it("getHelpSections excludes hidden commands", () => {
    const sections = getHelpSections();
    const allEntries = sections.flatMap(s => s.entries);
    const keys = allEntries.map(e => e.key);
    expect(keys).toContain("j / k");
    expect(keys).not.toContain("k");
    expect(keys).toContain("l / Enter");
    expect(keys).not.toContain("Enter");
  });

  it("getHelpSections includes externally handled commands", () => {
    const sections = getHelpSections();
    const allKeys = sections.flatMap(s => s.entries.map(e => e.key));
    expect(allKeys).toContain("Esc");
    expect(allKeys).toContain("⌘s");
    expect(allKeys).toContain("⌘k");
  });

  it("buildKeyMap excludes external commands", () => {
    const map = buildKeyMap();
    expect(map.has("Esc")).toBe(false);
    expect(map.has("⌘s")).toBe(false);
    expect(map.has("⌘k")).toBe(false);
  });

  it("buildKeyMap includes all internal command keys", () => {
    const map = buildKeyMap();
    expect(map.get("j")).toBe("navigate-next");
    expect(map.get("k")).toBe("navigate-prev");
    expect(map.get("l")).toBe("expand-collapse");
    expect(map.get("Enter")).toBe("expand-collapse");
    expect(map.get("?")).toBe("toggle-help");
  });

  it("buildKeyMap for development includes dev commands but not agents commands", () => {
    const map = buildKeyMap("development");
    expect(map.has("c")).toBe(true); // create-session (dev)
    expect(map.get("c")).toBe("create-session");
    expect(map.has("a")).toBe(false);
    expect(map.has("A")).toBe(false);
    expect(map.has("x")).toBe(false);
    expect(map.has("X")).toBe(false);
    expect(map.has("C")).toBe(false);
    expect(map.has("j")).toBe(true); // global nav
    expect(map.has("o")).toBe(false); // toggle-mode removed
  });

  it("buildKeyMap for agents includes agents commands but not dev commands", () => {
    const map = buildKeyMap("agents");
    expect(map.has("j")).toBe(true); // global nav
    expect(map.has("o")).toBe(true); // toggle-agent (agents)
    expect(map.get("o")).toBe("toggle-agent");
    expect(map.has("n")).toBe(false); // new-project is dev-only
  });

  it("buildKeyMap for notes includes notes commands but not dev or agents commands", () => {
    const map = buildKeyMap("notes");
    expect(map.has("j")).toBe(true);  // global nav
    expect(map.has("n")).toBe(true);  // create-note (notes)
    expect(map.get("n")).toBe("create-note");
    expect(map.has("d")).toBe(true);  // delete-note (notes)
    expect(map.get("d")).toBe("delete-note");
    expect(map.has("r")).toBe(true);  // rename-note (notes)
    expect(map.get("r")).toBe("rename-note");
    expect(map.has("p")).toBe(true);  // toggle-note-preview (notes)
    expect(map.get("p")).toBe("toggle-note-preview");
    expect(map.has("c")).toBe(false); // create-session is dev-only
    expect(map.get("o")).toBe("expand-collapse"); // open note for editing
    expect(map.get("i")).toBe("expand-collapse"); // open note for editing
    expect(map.get("a")).toBe("expand-collapse"); // open note for editing
  });

  it("buildKeyMap without mode includes all non-external commands", () => {
    const map = buildKeyMap();
    expect(map.has("j")).toBe(true);
    expect(map.has("c")).toBe(true);
    expect(map.has("o")).toBe(true);
  });

  it("help sections have correct entry counts for development mode", () => {
    const sections = getHelpSections("development");

    const essentials = sections.find(s => s.label === "Essentials")!;
    expect(essentials.entries).toHaveLength(9);
    expect(essentials.entries.map(e => e.key)).toEqual(["c", "j / k", "n", "d", "m", "f", "l / Enter", "Esc", "Esc Esc"]);

    expect(sections.find(s => s.label === "Navigation")).toBeUndefined();

    const sess = sections.find(s => s.label === "Sessions")!;
    expect(sess.entries).toHaveLength(4); // P, p, v, ⌘t
    expect(sess.entries.map(entry => entry.key)).toContain("⌘t");

    const proj = sections.find(s => s.label === "Projects")!;
    expect(proj.entries).toHaveLength(1); // i (open-issues-modal)

    const panels = sections.find(s => s.label === "Panels")!;
    expect(panels.entries).toHaveLength(2); // ?, ⌘k

    const debug = sections.find(s => s.label === "Debug")!;
    expect(debug.entries).toHaveLength(3); // ⌘s, ⌘d, ⌘S/⌘D
  });

  it("help sections have correct entry counts for agents mode", () => {
    const sections = getHelpSections("agents");
    const nav = sections.find(s => s.label === "Navigation")!;
    expect(nav.entries).toHaveLength(5);

    const sess = sections.find(s => s.label === "Sessions")!;
    expect(sess.entries).toHaveLength(3);

    const panels = sections.find(s => s.label === "Panels")!;
    expect(panels.entries).toHaveLength(2);

    const agents = sections.find(s => s.label === "Agents")!;
    expect(agents.entries).toHaveLength(4);
  });

  it("help sections have correct entry counts for notes mode", () => {
    const sections = getHelpSections("notes");
    const nav = sections.find(s => s.label === "Navigation")!;
    expect(nav.entries).toHaveLength(5);

    const sess = sections.find(s => s.label === "Sessions")!;
    expect(sess.entries).toHaveLength(3);

    const panels = sections.find(s => s.label === "Panels")!;
    expect(panels.entries).toHaveLength(2);

    const notes = sections.find(s => s.label === "Notes")!;
    expect(notes.entries).toHaveLength(5);
    expect(notes.entries).toContainEqual({
      key: "p",
      description: "Cycle edit / preview / split",
    });
  });

  it("removed commands are not in the registry", () => {
    const ids = commands.map(c => c.id);
    expect(ids).not.toContain("jump-mode");
    expect(ids).not.toContain("toggle-maintainer-panel");
    expect(ids).not.toContain("trigger-maintainer-check");
    expect(ids).not.toContain("clear-maintainer-reports");
  });

  it("new agents commands are in the registry", () => {
    const ids = commands.map(c => c.id);
    expect(ids).toContain("toggle-agent");
    expect(ids).toContain("trigger-agent-check");
    expect(ids).toContain("clear-agent-reports");
  });

  it("removed session-provider split commands are not in the registry", () => {
    const ids = commands.map(c => c.id);
    expect(ids).not.toContain("create-session-claude");
    expect(ids).not.toContain("create-session-codex");
    expect(ids).not.toContain("background-worker-claude");
    expect(ids).not.toContain("background-worker-codex");
  });

  it("includes toggle-maintainer-view command in agents mode", () => {
    const keyMap = buildKeyMap("agents");
    expect(keyMap.get("t")).toBe("toggle-maintainer-view");
  });

  it("includes deploy-project command in infrastructure mode keymap", () => {
    const map = buildKeyMap("infrastructure");
    expect(map.get("d")).toBe("deploy-project");
  });

  it("includes Infrastructure section in help for infrastructure mode", () => {
    const sections = getHelpSections("infrastructure");
    const infraSection = sections.find(s => s.label === "Infrastructure");
    expect(infraSection).toBeTruthy();
    expect(infraSection!.entries.length).toBeGreaterThan(0);
  });

  it("does not include infrastructure commands in development mode", () => {
    const map = buildKeyMap("development");
    expect(map.get("d")).not.toBe("deploy-project");
  });

  it("getHelpSections returns sections for infrastructure mode", () => {
    const sections = getHelpSections("infrastructure");
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Panels", "Infrastructure"]);
  });
});

describe("applyOverrides", () => {
  it("returns defaults when no overrides", () => {
    const result = applyOverrides(commands, {});
    expect(result).toBe(commands); // same reference
  });

  it("overrides a single key", () => {
    const result = applyOverrides(commands, { "navigate-next": "h" });
    const cmd = result.find((c) => c.id === "navigate-next" && !c.hidden);
    expect(cmd?.key).toBe("h");
  });

  it("does not modify hidden aliases", () => {
    const result = applyOverrides(commands, { "expand-collapse": "x" });
    const hidden = result.filter(
      (c) => c.id === "expand-collapse" && c.hidden,
    );
    for (const h of hidden) {
      const original = commands.find((c) => c.id === h.id && c.key === h.key);
      expect(original).toBeDefined();
    }
  });

  it("overrides Meta+ commands", () => {
    const result = applyOverrides(commands, { screenshot: "Meta+x" });
    const cmd = result.find((c) => c.id === "screenshot" && !c.hidden);
    expect(cmd?.key).toBe("Meta+x");
  });

  it("ignores unknown command IDs", () => {
    const result = applyOverrides(commands, { nonexistent: "x" });
    expect(result).toEqual(commands);
  });

  it("clears helpKey when override is applied", () => {
    const result = applyOverrides(commands, { "navigate-next": "h" });
    const cmd = result.find((c) => c.id === "navigate-next" && !c.hidden);
    expect(cmd?.helpKey).toBeUndefined();
  });

  it("applies override to hidden-but-sole commands like navigate-prev", () => {
    const result = applyOverrides(commands, { "navigate-prev": "g" });
    const cmd = result.find((c) => c.id === "navigate-prev");
    expect(cmd?.key).toBe("g");
  });

  it("still skips hidden entries that are true aliases (non-hidden sibling exists)", () => {
    const result = applyOverrides(commands, { "expand-collapse": "x" });
    // The non-hidden entry should be overridden
    const nonHidden = result.find((c) => c.id === "expand-collapse" && !c.hidden);
    expect(nonHidden?.key).toBe("x");
    // The hidden Enter alias should keep its original key
    const hiddenEnter = result.find((c) => c.id === "expand-collapse" && c.hidden && c.key === "Enter");
    expect(hiddenEnter).toBeDefined();
  });
});

describe("buildKeyMap with overrides", () => {
  it("uses overridden key", () => {
    const resolved = applyOverrides(commands, { "navigate-next": "h" });
    const map = buildKeyMap("development", resolved);
    expect(map.get("h")).toBe("navigate-next");
    expect(map.has("j")).toBe(false);
  });
});

describe("formatDisplayKey", () => {
  it("converts Meta+ to ⌘ for cmd", () => {
    expect(formatDisplayKey("Meta+c", "cmd")).toBe("⌘c");
  });

  it("converts Meta+ to ⌃ for ctrl", () => {
    expect(formatDisplayKey("Meta+c", "ctrl")).toBe("⌃c");
  });

  it("passes through bare keys unchanged", () => {
    expect(formatDisplayKey("j", "cmd")).toBe("j");
  });

  it("passes through existing ⌘ prefix unchanged for cmd", () => {
    expect(formatDisplayKey("⌘s", "cmd")).toBe("⌘s");
  });

  it("converts legacy ⌘ to ⌃ when meta is ctrl", () => {
    expect(formatDisplayKey("⌘s", "ctrl")).toBe("⌃s");
  });

  it("converts composite ⌘ strings when meta is ctrl", () => {
    expect(formatDisplayKey("⌘S / ⌘D", "ctrl")).toBe("⌃S / ⌃D");
  });
});

describe("getHelpSections with metaKey", () => {
  it("formats Meta+ overrides as ⌘ in help display", () => {
    const resolved = applyOverrides(commands, { "create-session": "Meta+c" });
    const sections = getHelpSections("development", resolved, "cmd");
    const essentials = sections.find(s => s.label === "Essentials")!;
    const createEntry = essentials.entries.find(e => e.description === "Create session")!;
    expect(createEntry.key).toBe("⌘c");
  });

  it("formats Meta+ overrides as ⌃ when meta is ctrl", () => {
    const resolved = applyOverrides(commands, { "create-session": "Meta+c" });
    const sections = getHelpSections("development", resolved, "ctrl");
    const essentials = sections.find(s => s.label === "Essentials")!;
    const createEntry = essentials.entries.find(e => e.description === "Create session")!;
    expect(createEntry.key).toBe("⌃c");
  });

  it("preserves existing ⌘ keys in default display", () => {
    const sections = getHelpSections("development", undefined, "cmd");
    const allKeys = sections.flatMap(s => s.entries.map(e => e.key));
    expect(allKeys).toContain("⌘s");
    expect(allKeys).toContain("⌘k");
  });

  it("converts legacy ⌘ keys to ⌃ when meta is ctrl", () => {
    const sections = getHelpSections("development", undefined, "ctrl");
    const allKeys = sections.flatMap(s => s.entries.map(e => e.key));
    expect(allKeys).toContain("⌃s");
    expect(allKeys).toContain("⌃k");
    expect(allKeys).not.toContain("⌘s");
    expect(allKeys).not.toContain("⌘k");
  });
});

describe("getHelpSections composed keys after overrides", () => {
  it("non-dev mode shows composed navigate keys after override", () => {
    const resolved = applyOverrides(commands, { "navigate-next": "h", "navigate-prev": "g" });
    const sections = getHelpSections("agents", resolved);
    const nav = sections.find(s => s.label === "Navigation")!;
    const navEntry = nav.entries.find(e => e.description.includes("Next / previous"));
    expect(navEntry?.key).toBe("h / g");
  });

  it("non-dev mode shows composed expand-collapse keys after override", () => {
    const resolved = applyOverrides(commands, { "expand-collapse": "x" });
    const sections = getHelpSections("agents", resolved);
    const nav = sections.find(s => s.label === "Navigation")!;
    const expandEntry = nav.entries.find(e => e.description.includes("Expand/collapse"));
    expect(expandEntry?.key).toBe("x / Enter");
  });

  it("non-dev mode shows dynamic screenshot-picker key after override", () => {
    const resolved = applyOverrides(commands, { screenshot: "Meta+x", "screenshot-cropped": "Meta+y" });
    const sections = getHelpSections("agents", resolved, "cmd");
    const sess = sections.find(s => s.label === "Sessions")!;
    const pickerEntry = sess.entries.find(e => e.description === "Screenshot → pick session");
    expect(pickerEntry?.key).toBe("⌘X / ⌘Y");
  });

  it("dev mode debug section shows dynamic screenshot-picker key after override", () => {
    const resolved = applyOverrides(commands, { screenshot: "Meta+x", "screenshot-cropped": "Meta+y" });
    const sections = getHelpSections("development", resolved, "cmd");
    const debug = sections.find(s => s.label === "Debug")!;
    const pickerEntry = debug.entries.find(e => e.description === "Screenshot → pick session");
    expect(pickerEntry?.key).toBe("⌘X / ⌘Y");
  });

  it("screenshot-picker key defaults to ⌘S / ⌘D with no overrides", () => {
    const sections = getHelpSections("agents");
    const sess = sections.find(s => s.label === "Sessions")!;
    const pickerEntry = sess.entries.find(e => e.description === "Screenshot → pick session");
    expect(pickerEntry?.key).toBe("⌘S / ⌘D");
  });
});
