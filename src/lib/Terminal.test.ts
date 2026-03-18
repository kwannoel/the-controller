import { render } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { command, listen } from "$lib/backend";

const { terminalInstances, terminalConstructor } = vi.hoisted(() => ({
  terminalInstances: [] as any[],
  terminalConstructor: vi.fn(),
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: class MockXtermTerminal {
    options: any;
    rows = 24;
    cols = 80;
    buffer = { active: { type: "normal", viewportY: 100, baseY: 100 } };

    constructor(options?: unknown) {
      this.options = options;
      terminalInstances.push(this);
      terminalConstructor(options);
    }

    loadAddon() {}
    open() {}
    attachCustomKeyEventHandler() {}
    attachCustomWheelEventHandler() {}
    onData() {}
    onScroll() {}
    write() {}
    writeln() {}
    refresh() {}
    scrollToBottom() {}
    scrollToLine() {}
    focus() {}
    dispose() {}
  },
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class MockFitAddon {
    fit() {}
  },
}));

vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: class MockWebLinksAddon {},
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(),
}));

import Terminal from "./Terminal.svelte";

// Scroll-preservation logic is tested directly in terminal-scroll.test.ts.
// These tests focus on component-level concerns (theme loading, mounting).

describe("Terminal theme loading", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    terminalInstances.length = 0;
    vi.mocked(command).mockResolvedValue(undefined);
    vi.mocked(listen).mockReturnValue(() => {});

    class MockResizeObserver {
      observe() {}
      disconnect() {}
    }

    class MockMutationObserver {
      observe() {}
      disconnect() {}
    }

    vi.stubGlobal("ResizeObserver", MockResizeObserver);
    vi.stubGlobal("MutationObserver", MockMutationObserver);
  });

  it("uses the backend-provided theme when creating xterm", async () => {
    vi.mocked(command).mockImplementation(async (cmd) => {
      if (cmd === "load_terminal_theme") {
        return {
          background: "#111111",
          foreground: "#eeeeee",
          cursor: "#ffcc00",
          selectionBackground: "#333333",
        };
      }
      return undefined;
    });

    render(Terminal, { sessionId: "sess-1" });
    await vi.dynamicImportSettled();

    expect(command).toHaveBeenCalledWith("load_terminal_theme");
    expect(terminalConstructor).toHaveBeenCalled();
    expect(terminalInstances[0]?.options?.theme).toEqual({
      background: "#111111",
      foreground: "#eeeeee",
      cursor: "#ffcc00",
      selectionBackground: "#333333",
    });
  });

  it("falls back to the built-in theme when theme loading fails", async () => {
    vi.mocked(command).mockImplementation(async (cmd) => {
      if (cmd === "load_terminal_theme") {
        throw new Error("boom");
      }
      return undefined;
    });

    render(Terminal, { sessionId: "sess-1" });
    await vi.dynamicImportSettled();

    expect(terminalConstructor).toHaveBeenCalled();
    expect(terminalInstances[0]?.options?.theme).toEqual({
      background: "#000000",
      foreground: "#e0e0e0",
      cursor: "#ffffff",
      selectionBackground: "#2e2e2e",
    });
  });
});
