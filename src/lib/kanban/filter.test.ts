// @vitest-environment node
import { describe, it, expect } from "vitest";
import type { GithubIssue } from "../stores";
import { applyFilters, type Filters } from "./filter";

function issue(
  number: number,
  labels: string[],
  assignees: string[] = [],
  milestone: string | null = null,
): GithubIssue {
  return {
    number,
    title: `issue ${number}`,
    url: `https://x/${number}`,
    labels: labels.map((name) => ({ name })),
    assignees: assignees.map((login) => ({ login })),
    milestone: milestone ? { title: milestone } : null,
  };
}

const empty: Filters = {
  assignees: new Set(),
  labels: new Set(),
  milestone: null,
};

describe("applyFilters", () => {
  const issues: GithubIssue[] = [
    issue(1, ["bug"], ["alice"], "v1"),
    issue(2, ["feature"], ["bob"], "v1"),
    issue(3, ["bug", "priority:high"], ["alice", "bob"], "v2"),
    issue(4, [], [], null),
  ];

  it("returns all issues when filters are empty", () => {
    expect(applyFilters(issues, empty).map((i) => i.number)).toEqual([
      1, 2, 3, 4,
    ]);
  });

  it("filters by assignee (any-of within the set)", () => {
    const f: Filters = { ...empty, assignees: new Set(["alice"]) };
    expect(applyFilters(issues, f).map((i) => i.number)).toEqual([1, 3]);
  });

  it("filters by label (all-of within the set)", () => {
    const f: Filters = {
      ...empty,
      labels: new Set(["bug", "priority:high"]),
    };
    expect(applyFilters(issues, f).map((i) => i.number)).toEqual([3]);
  });

  it("filters by milestone (exact match)", () => {
    const f: Filters = { ...empty, milestone: "v2" };
    expect(applyFilters(issues, f).map((i) => i.number)).toEqual([3]);
  });

  it("intersects assignee + label + milestone", () => {
    const f: Filters = {
      assignees: new Set(["alice"]),
      labels: new Set(["bug"]),
      milestone: "v1",
    };
    expect(applyFilters(issues, f).map((i) => i.number)).toEqual([1]);
  });

  it("status:* labels in the filter set are ignored", () => {
    const f: Filters = { ...empty, labels: new Set(["status:done"]) };
    expect(applyFilters(issues, f).map((i) => i.number)).toEqual([
      1, 2, 3, 4,
    ]);
  });
});
