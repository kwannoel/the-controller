// @vitest-environment node
import { describe, it, expect } from "vitest";
import type { GithubIssue } from "../stores";
import {
  applyOrdering,
  moveIssue,
  orderKey,
  type OrderMap,
} from "./ordering";

function issue(n: number): GithubIssue {
  return {
    number: n,
    title: `issue ${n}`,
    url: `https://x/${n}`,
    labels: [],
  };
}

const repo = "/tmp/repo";

describe("orderKey", () => {
  it("combines repoPath and column", () => {
    expect(orderKey(repo, "todo")).toBe(`${repo}:todo`);
  });
});

describe("applyOrdering", () => {
  it("returns input order when no saved order exists", () => {
    const issues = [issue(3), issue(1), issue(2)];
    expect(
      applyOrdering(issues, "todo", repo, {}).map((i) => i.number),
    ).toEqual([3, 1, 2]);
  });

  it("orders by saved sequence, then appends new issues at the end", () => {
    const issues = [issue(1), issue(2), issue(3), issue(4)];
    const order: OrderMap = { [orderKey(repo, "todo")]: [3, 1] };
    expect(
      applyOrdering(issues, "todo", repo, order).map((i) => i.number),
    ).toEqual([3, 1, 2, 4]);
  });

  it("drops stale numbers that are no longer in the input", () => {
    const issues = [issue(1), issue(2)];
    const order: OrderMap = { [orderKey(repo, "todo")]: [99, 1, 100, 2] };
    expect(
      applyOrdering(issues, "todo", repo, order).map((i) => i.number),
    ).toEqual([1, 2]);
  });
});

describe("moveIssue", () => {
  it("reorders within a column", () => {
    const order: OrderMap = { [orderKey(repo, "todo")]: [1, 2, 3] };
    const next = moveIssue(
      order,
      orderKey(repo, "todo"),
      orderKey(repo, "todo"),
      3,
      0,
    );
    expect(next[orderKey(repo, "todo")]).toEqual([3, 1, 2]);
  });

  it("moves across columns", () => {
    const order: OrderMap = {
      [orderKey(repo, "todo")]: [1, 2, 3],
      [orderKey(repo, "done")]: [9],
    };
    const next = moveIssue(
      order,
      orderKey(repo, "todo"),
      orderKey(repo, "done"),
      2,
      0,
    );
    expect(next[orderKey(repo, "todo")]).toEqual([1, 3]);
    expect(next[orderKey(repo, "done")]).toEqual([2, 9]);
  });

  it("inserts at the end when toIndex exceeds length", () => {
    const order: OrderMap = {
      [orderKey(repo, "todo")]: [1, 2],
      [orderKey(repo, "done")]: [],
    };
    const next = moveIssue(
      order,
      orderKey(repo, "todo"),
      orderKey(repo, "done"),
      1,
      99,
    );
    expect(next[orderKey(repo, "done")]).toEqual([1]);
  });

  it("does not mutate the input map", () => {
    const order: OrderMap = { [orderKey(repo, "todo")]: [1, 2] };
    const snapshot = JSON.stringify(order);
    moveIssue(
      order,
      orderKey(repo, "todo"),
      orderKey(repo, "todo"),
      2,
      0,
    );
    expect(JSON.stringify(order)).toBe(snapshot);
  });
});
