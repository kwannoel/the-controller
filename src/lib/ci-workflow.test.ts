import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("CI workflow", () => {
  it("installs Linux audio headers needed by Rust voice dependencies", () => {
    const workflow = readFileSync(resolve(process.cwd(), ".github/workflows/ci.yml"), "utf8");

    expect(workflow).toMatch(/apt-get install -y[\s\S]*libasound2-dev/);
  });
});
