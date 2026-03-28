import { describe, it, expect } from "vitest";
import { commands, getHelpSections, buildKeyMap } from "./commands";

describe("command registry", () => {
  it("every non-external command has a unique key within its mode", () => {
    const internal = commands.filter(c => !c.handledExternally);
    const globalKeys = internal.filter(c => !c.mode).map(c => c.key);
    const globalSet = new Set(globalKeys);
    expect(globalKeys.length).toBe(globalSet.size);

    const modes = ["development", "agents"] as const;
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

  it("getHelpSections without mode returns all sections", () => {
    const sections = getHelpSections();
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Projects", "Panels", "Agents"]);
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
    expect(sess.entries).toHaveLength(6); // y, e, P, p, v, ⌘t
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

});
