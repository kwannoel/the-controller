import { describe, it, expect, vi } from "vitest";
import { makeCustomKeyHandler } from "./terminal-keys";

function makeEvent(overrides: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    type: "keydown",
    key: "",
    shiftKey: false,
    metaKey: false,
    ctrlKey: false,
    ...overrides,
  } as unknown as KeyboardEvent;
}

describe("makeCustomKeyHandler", () => {
  it("sends CSI u sequence on Shift+Enter keydown", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "Enter", shiftKey: true, type: "keydown" }));

    expect(result).toBe(false);
    expect(write).toHaveBeenCalledWith("\x1b[13;2u");
  });

  it("blocks Shift+Enter on keypress without sending data", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "Enter", shiftKey: true, type: "keypress" }));

    expect(result).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("blocks Shift+Enter on keyup without sending data", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "Enter", shiftKey: true, type: "keyup" }));

    expect(result).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("allows regular Enter through", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "Enter", shiftKey: false, type: "keydown" }));

    expect(result).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("does not intercept Cmd-V (paste handled natively by xterm)", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "v", metaKey: true, type: "keydown" }));

    expect(result).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("allows non-Shift+Enter keys through", () => {
    const write = vi.fn();
    const handler = makeCustomKeyHandler(write);

    const result = handler(makeEvent({ key: "a", type: "keypress" }));

    expect(result).toBe(true);
  });

  describe("image paste handling", () => {
    it("blocks Cmd-V on keydown and calls onImagePaste", () => {
      const write = vi.fn();
      const onImagePaste = vi.fn();
      const handler = makeCustomKeyHandler(write, { onImagePaste });

      const result = handler(makeEvent({ key: "v", metaKey: true, type: "keydown" }));

      expect(result).toBe(false);
      expect(onImagePaste).toHaveBeenCalledOnce();
    });

    it("blocks Cmd-V on keyup without calling onImagePaste", () => {
      const write = vi.fn();
      const onImagePaste = vi.fn();
      const handler = makeCustomKeyHandler(write, { onImagePaste });

      const result = handler(makeEvent({ key: "v", metaKey: true, type: "keyup" }));

      expect(result).toBe(false);
      expect(onImagePaste).not.toHaveBeenCalled();
    });

    it("blocks Ctrl-V on keydown and calls onImagePaste", () => {
      const write = vi.fn();
      const onImagePaste = vi.fn();
      const handler = makeCustomKeyHandler(write, { onImagePaste });

      const result = handler(makeEvent({ key: "v", ctrlKey: true, type: "keydown" }));

      expect(result).toBe(false);
      expect(onImagePaste).toHaveBeenCalledOnce();
    });

    it("lets Cmd-V through when no onImagePaste callback", () => {
      const write = vi.fn();
      const handler = makeCustomKeyHandler(write);

      const result = handler(makeEvent({ key: "v", metaKey: true, type: "keydown" }));

      expect(result).toBe(true);
    });
  });
});
