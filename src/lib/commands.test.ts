import { describe, it, expect } from "vitest";
import { commands, getHelpSections, buildKeyMap } from "./commands";

describe("command registry", () => {
  it("every non-external command has a unique key", () => {
    const internal = commands.filter(c => !c.handledExternally);
    const keys = internal.map(c => c.key);
    const unique = new Set(keys);
    expect(keys.length).toBe(unique.size);
  });

  it("every non-hidden command has a description", () => {
    for (const cmd of commands.filter(c => !c.hidden)) {
      expect(cmd.description.length).toBeGreaterThan(0);
    }
  });

  it("getHelpSections returns all four sections in order", () => {
    const sections = getHelpSections();
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Projects", "Panels"]);
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
    expect(allKeys).toContain("⌘S");
    expect(allKeys).toContain("⌘K");
  });

  it("buildKeyMap excludes external commands", () => {
    const map = buildKeyMap();
    expect(map.has("Esc")).toBe(false);
    expect(map.has("⌘S")).toBe(false);
    expect(map.has("⌘K")).toBe(false);
  });

  it("buildKeyMap includes all internal command keys", () => {
    const map = buildKeyMap();
    expect(map.get("j")).toBe("navigate-next");
    expect(map.get("k")).toBe("navigate-prev");
    expect(map.get("l")).toBe("expand-collapse");
    expect(map.get("Enter")).toBe("expand-collapse");
    expect(map.get("?")).toBe("toggle-help");
  });

  it("help sections match the original hardcoded sections", () => {
    const sections = getHelpSections();
    const nav = sections.find(s => s.label === "Navigation")!;
    expect(nav.entries).toHaveLength(7);

    const sess = sections.find(s => s.label === "Sessions")!;
    expect(sess.entries).toHaveLength(8);

    const proj = sections.find(s => s.label === "Projects")!;
    expect(proj.entries).toHaveLength(7);

    const panels = sections.find(s => s.label === "Panels")!;
    expect(panels.entries).toHaveLength(6);
  });
});
