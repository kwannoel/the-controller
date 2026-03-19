import { describe, expect, it } from "vitest";
import { extractLineText, findUrlAtPosition } from "./terminal-links";

describe("findUrlAtPosition", () => {
  it("returns URL when column is within URL bounds", () => {
    const text = "Visit https://example.com for more";
    expect(findUrlAtPosition(text, 6)).toBe("https://example.com");
    expect(findUrlAtPosition(text, 24)).toBe("https://example.com");
  });

  it("returns null when column is outside URL bounds", () => {
    const text = "Visit https://example.com for more";
    expect(findUrlAtPosition(text, 0)).toBeNull();
    expect(findUrlAtPosition(text, 25)).toBeNull();
  });

  it("returns null when no URL present", () => {
    expect(findUrlAtPosition("no links here", 5)).toBeNull();
  });

  it("matches the correct URL when multiple URLs on a line", () => {
    const text = "See https://a.com and https://b.com ok";
    expect(findUrlAtPosition(text, 4)).toBe("https://a.com");
    expect(findUrlAtPosition(text, 22)).toBe("https://b.com");
  });

  it("handles http URLs", () => {
    const text = "Go to http://insecure.example.com now";
    expect(findUrlAtPosition(text, 10)).toBe("http://insecure.example.com");
  });

  it("handles URLs with paths and query params", () => {
    const text = "Link: https://example.com/path?q=1&r=2#frag done";
    expect(findUrlAtPosition(text, 10)).toBe(
      "https://example.com/path?q=1&r=2#frag",
    );
  });
});

describe("extractLineText", () => {
  it("extracts characters from buffer cells", () => {
    const line = {
      length: 5,
      getCell(x: number) {
        return { getChars: () => "hello"[x] };
      },
    };
    expect(extractLineText(line)).toBe("hello");
  });

  it("replaces empty cells with spaces", () => {
    const chars = ["a", "", "b"];
    const line = {
      length: 3,
      getCell(x: number) {
        return { getChars: () => chars[x] };
      },
    };
    expect(extractLineText(line)).toBe("a b");
  });

  it("handles undefined cells as spaces", () => {
    const line = {
      length: 3,
      getCell() {
        return undefined;
      },
    };
    expect(extractLineText(line)).toBe("   ");
  });
});
