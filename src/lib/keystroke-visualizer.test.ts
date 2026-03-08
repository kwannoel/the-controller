import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { get } from "svelte/store";
import {
  keystrokeVisualizerEnabled,
  keystrokes,
  toggleKeystrokeVisualizer,
  pushKeystroke,
} from "./keystroke-visualizer";

describe("keystroke-visualizer", () => {
  beforeEach(() => {
    keystrokeVisualizerEnabled.set(false);
    keystrokes.set([]);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("toggles enabled state", () => {
    expect(get(keystrokeVisualizerEnabled)).toBe(false);
    toggleKeystrokeVisualizer();
    expect(get(keystrokeVisualizerEnabled)).toBe(true);
    toggleKeystrokeVisualizer();
    expect(get(keystrokeVisualizerEnabled)).toBe(false);
  });

  it("pushKeystroke adds a keystroke when enabled", () => {
    keystrokeVisualizerEnabled.set(true);
    pushKeystroke("j");
    const list = get(keystrokes);
    expect(list).toHaveLength(1);
    expect(list[0].label).toBe("j");
  });

  it("pushKeystroke is a no-op when disabled", () => {
    pushKeystroke("j");
    expect(get(keystrokes)).toHaveLength(0);
  });

  it("auto-removes keystroke after 2 seconds", () => {
    keystrokeVisualizerEnabled.set(true);
    pushKeystroke("k");
    expect(get(keystrokes)).toHaveLength(1);
    vi.advanceTimersByTime(2000);
    expect(get(keystrokes)).toHaveLength(0);
  });

  it("keeps max 5 keystrokes", () => {
    keystrokeVisualizerEnabled.set(true);
    for (let i = 0; i < 7; i++) {
      pushKeystroke(String(i));
    }
    expect(get(keystrokes)).toHaveLength(5);
    // oldest are dropped
    expect(get(keystrokes)[0].label).toBe("2");
  });

  it("clears keystrokes when toggled off", () => {
    keystrokeVisualizerEnabled.set(true);
    pushKeystroke("j");
    expect(get(keystrokes)).toHaveLength(1);
    toggleKeystrokeVisualizer(); // toggles off
    expect(get(keystrokes)).toHaveLength(0);
  });
});
