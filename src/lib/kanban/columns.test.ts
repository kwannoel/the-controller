// @vitest-environment node
import { describe, it, expect } from "vitest";
import type { GithubIssue } from "../stores";
import {
  COLUMNS,
  LABEL_BY_COLUMN,
  columnForIssue,
  type Column,
} from "./columns";

function issue(labels: string[]): GithubIssue {
  return {
    number: 1,
    title: "t",
    url: "https://x",
    labels: labels.map((name) => ({ name })),
  };
}

describe("COLUMNS", () => {
  it("has the five columns in left-to-right order", () => {
    expect(COLUMNS).toEqual([
      "backlog",
      "todo",
      "in-progress",
      "in-review",
      "done",
    ] satisfies Column[]);
  });

  it("has a status:* label for every column", () => {
    for (const c of COLUMNS) {
      expect(LABEL_BY_COLUMN[c]).toMatch(/^status:/);
    }
  });
});

describe("columnForIssue", () => {
  it("maps each status:* label to its column", () => {
    for (const c of COLUMNS) {
      expect(columnForIssue(issue([LABEL_BY_COLUMN[c]]))).toBe(c);
    }
  });

  it("defaults to backlog when no status label is present", () => {
    expect(columnForIssue(issue([]))).toBe("backlog");
    expect(columnForIssue(issue(["priority:high", "complexity:low"]))).toBe(
      "backlog",
    );
  });

  it("ignores non-status labels even if they look similar", () => {
    expect(columnForIssue(issue(["in-progress"]))).toBe("backlog");
  });

  it("when multiple status labels are set, prefers the rightmost column", () => {
    const i = issue([
      LABEL_BY_COLUMN["todo"],
      LABEL_BY_COLUMN["done"],
      LABEL_BY_COLUMN["in-progress"],
    ]);
    expect(columnForIssue(i)).toBe("done");
  });
});
