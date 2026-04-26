import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { globSync } from "glob";

function read(path: string): string {
  return readFileSync(path, "utf8");
}

function serverRoutes(): Set<string> {
  const source = read("server/src/main.rs");
  return new Set([...source.matchAll(/"\/api\/([a-zA-Z0-9_]+)"/g)].map((m) => m[1]));
}

function productionFrontendCommands(): Set<string> {
  const files = globSync("src/**/*.{ts,svelte}", {
    ignore: ["src/**/*.test.ts", "src/**/*.test.svelte.ts", "src/**/__mocks__/**"],
  });
  const commands = new Set<string>();

  for (const file of files) {
    const source = read(file);
    for (const match of source.matchAll(/command(?:<[^>]+>)?\(\s*["']([a-zA-Z0-9_]+)["']/g)) {
      commands.add(match[1]);
    }
  }

  return commands;
}

describe("web backend migration audit", () => {
  it("keeps every production frontend command backed by an HTTP route", () => {
    const routes = serverRoutes();
    const missing = [...productionFrontendCommands()].filter((cmd) => !routes.has(cmd));

    expect(missing).toEqual([]);
  });

  it("keeps the remaining browser-native replacement wired", () => {
    const nativeReplacements = read("src/lib/native.ts");
    expect(nativeReplacements).toContain("ClipboardItem");
  });

  it("keeps active docs aligned with the web frontend plus backend runtime", () => {
    const activeDocs = [
      "README.md",
      "ARCHITECTURE.md",
      "docs/keyboard-modes.md",
      "e2e/specs/chat-mode.spec.ts",
    ];

    const stalePatterns = [
      /Built with Tauri/i,
      /Tauri v2/i,
      /npm run tauri dev/i,
      /src-tauri\/src/i,
      /six workspace modes/i,
      /four workspace modes/i,
      /Development . manage sessions/i,
      /Ambient Mode . Architecture Keys/i,
      /Ambient Mode . Notes Keys/i,
      /Ambient Mode . Infrastructure Keys/i,
      /Ambient Mode . Voice Keys/i,
      /read_daemon_token`[\s\S]{0,120}not exposed/i,
    ];

    const stale = activeDocs.flatMap((file) => {
      const source = read(file);
      return stalePatterns
        .filter((pattern) => pattern.test(source))
        .map((pattern) => `${file}: ${pattern}`);
    });

    expect(stale).toEqual([]);
  });
});
