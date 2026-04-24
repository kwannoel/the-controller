import { describe, it, expect, vi, afterEach } from "vitest";
import { clipboardHasImage } from "./clipboard";

const originalClipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function stubRead(impl: () => Promise<any>) {
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { read: vi.fn(impl) },
  });
}

afterEach(() => {
  if (originalClipboard) {
    Object.defineProperty(navigator, "clipboard", originalClipboard);
  }
});

describe("clipboardHasImage", () => {
  it("returns true when clipboard contains an image", async () => {
    stubRead(async () => [{ types: ["image/png"] }]);
    expect(await clipboardHasImage()).toBe(true);
  });

  it("returns false when clipboard has only text", async () => {
    stubRead(async () => [{ types: ["text/plain"] }]);
    expect(await clipboardHasImage()).toBe(false);
  });

  it("returns false when clipboard read fails", async () => {
    stubRead(async () => {
      throw new Error("denied");
    });
    expect(await clipboardHasImage()).toBe(false);
  });
});
