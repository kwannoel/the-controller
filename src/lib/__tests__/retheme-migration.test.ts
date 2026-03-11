import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { globSync } from "glob";
import { resolve } from "path";

const OLD_CATPPUCCIN_COLORS = [
  "#11111b",
  "#181825",
  "#1e1e2e",
  "#313244",
  "#45475a",
  "#585b70",
  "#6c7086",
  "#7f849c",
  "#a6adc8",
  "#bac2de",
  "#cdd6f4",
  "#89b4fa",
  "#f38ba8",
  "#a6e3a1",
  "#f9e2af",
  "#fab387",
  "#89dceb",
  "#cba6f7",
  "#eba0ac",
  "#f5e0dc",
];

describe("retheme migration", () => {
  it("no Catppuccin hex values remain in component styles", () => {
    const root = resolve(__dirname, "../../..");
    const files = globSync("src/**/*.svelte", { cwd: root });
    const violations: string[] = [];

    for (const file of files) {
      const content = readFileSync(resolve(root, file), "utf-8");
      const styleMatch = content.match(/<style[\s\S]*?>([\s\S]*?)<\/style>/);
      if (!styleMatch) continue;
      const styleBlock = styleMatch[1].toLowerCase();

      for (const color of OLD_CATPPUCCIN_COLORS) {
        if (styleBlock.includes(color)) {
          violations.push(`${file}: still contains ${color}`);
        }
      }
    }

    expect(violations).toEqual([]);
  });

  it("CSS custom properties are defined in app.css", () => {
    const root = resolve(__dirname, "../../..");
    const css = readFileSync(resolve(root, "src/app.css"), "utf-8");
    const requiredTokens = [
      "--bg-void",
      "--bg-base",
      "--bg-surface",
      "--bg-elevated",
      "--bg-hover",
      "--bg-active",
      "--border-subtle",
      "--border-default",
      "--text-emphasis",
      "--text-primary",
      "--text-secondary",
      "--text-tertiary",
      "--status-idle",
      "--status-working",
      "--status-error",
      "--status-exited",
      "--focus-ring",
    ];

    for (const token of requiredTokens) {
      expect(css).toContain(token);
    }
  });
});
