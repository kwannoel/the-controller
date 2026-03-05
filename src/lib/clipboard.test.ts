import { describe, it, expect, vi } from "vitest";
import { clipboardHasImage } from "./clipboard";

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  readImage: vi.fn(),
}));

import { readImage } from "@tauri-apps/plugin-clipboard-manager";
const mockReadImage = vi.mocked(readImage);

describe("clipboardHasImage", () => {
  it("returns true when clipboard contains an image", async () => {
    mockReadImage.mockResolvedValueOnce({} as any);
    expect(await clipboardHasImage()).toBe(true);
  });

  it("returns false when clipboard has no image", async () => {
    mockReadImage.mockRejectedValueOnce(new Error("No image"));
    expect(await clipboardHasImage()).toBe(false);
  });
});
