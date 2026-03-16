import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("CI workflow", () => {
  it("matches the repository CI contract", () => {
    const workflow = readFileSync(resolve(process.cwd(), ".github/workflows/ci.yml"), "utf8");

    expect(workflow).toMatch(/branches:\s*\[main\]/);
    expect(workflow).toMatch(/pnpm\/action-setup@/);
    expect(workflow).toMatch(/cache:\s*["']?pnpm["']?/);
    expect(workflow).toMatch(/pnpm install --frozen-lockfile/);
    expect(workflow).toMatch(/pnpm check/);
    expect(workflow).toMatch(/cargo fmt --check/);
    expect(workflow).toMatch(/cargo clippy -- -D warnings/);
    expect(workflow).toMatch(/cargo test/);
    expect(workflow).toMatch(/apt-get install -y[\s\S]*libasound2-dev/);
  });
});
