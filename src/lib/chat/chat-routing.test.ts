import { describe, expect, it } from "vitest";
import {
  extractRouteTokenQuery,
  insertRouteToken,
  reconcileRouteTokens,
  routeTokenForProfile,
} from "./chat-routing";

describe("extractRouteTokenQuery", () => {
  it("detects @ and % token queries at the cursor", () => {
    expect(extractRouteTokenQuery("ask @rev", 8)).toEqual({
      kind: "reusable",
      query: "rev",
      start: 4,
      end: 8,
    });
    expect(extractRouteTokenQuery("ask %debug", 10)).toEqual({
      kind: "shadow",
      query: "debug",
      start: 4,
      end: 10,
    });
  });
});

describe("insertRouteToken", () => {
  it("replaces the current token query and returns durable metadata", () => {
    expect(
      insertRouteToken("ask @rev now", { kind: "reusable", query: "rev", start: 4, end: 8 }, "reviewer"),
    ).toEqual({
      text: "ask @reviewer now",
      cursor: 13,
      token: { kind: "reusable", handle: "reviewer", start: 4, end: 13 },
    });
  });
});

describe("routeTokenForProfile", () => {
  it("maps a selected profile to an outbound route token", () => {
    expect(
      routeTokenForProfile(
        { handle: "debug" },
        "shadow",
        4,
        10,
      ),
    ).toEqual({ kind: "shadow", handle: "debug", start: 4, end: 10 });
  });
});

describe("reconcileRouteTokens", () => {
  it("updates shifted token spans and drops tokens no longer present", () => {
    expect(
      reconcileRouteTokens("please ask @reviewer", [
        { kind: "reusable", handle: "reviewer", start: 4, end: 13 },
        { kind: "shadow", handle: "debugger", start: 14, end: 23 },
      ]),
    ).toEqual([
      { kind: "reusable", handle: "reviewer", start: 11, end: 20 },
    ]);
  });

  it("drops selected tokens that were edited into longer handles", () => {
    expect(
      reconcileRouteTokens("ask %reviewerx", [
        { kind: "shadow", handle: "reviewer", start: 4, end: 13 },
      ]),
    ).toEqual([]);
  });
});
