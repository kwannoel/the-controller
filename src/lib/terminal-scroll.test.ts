import { describe, expect, it, vi } from "vitest";
import { createScrollTracker, type ScrollableTerminal, type Fittable } from "./terminal-scroll";

/** Create a mock terminal with configurable scroll position. */
function mockTerminal(viewportY = 100, baseY = 100) {
  return {
    buffer: { active: { viewportY, baseY } },
    scrollToBottom: vi.fn() as unknown as (() => void) & ReturnType<typeof vi.fn>,
    scrollToLine: vi.fn() as unknown as ((line: number) => void) & ReturnType<typeof vi.fn>,
  } satisfies ScrollableTerminal;
}

/** Create a mock FitAddon whose fit() can optionally mutate the terminal. */
function mockFitAddon(onFit?: () => void) {
  return {
    fit: vi.fn(() => onFit?.()) as unknown as (() => void) & ReturnType<typeof vi.fn>,
  } satisfies Fittable;
}

/** Create a mock HTMLElement with the given offsetParent. */
function mockElement(offsetParent: Element | null): HTMLElement {
  return { offsetParent } as unknown as HTMLElement;
}

describe("createScrollTracker", () => {
  describe("handleScroll", () => {
    it("marks user as scrolled up when viewportY < baseY", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(50, 100);
      const el = mockElement(document.body);

      tracker.handleScroll(term, el);

      expect(tracker.isUserScrolledUp()).toBe(true);
    });

    it("marks user as at bottom when viewportY === baseY", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(100, 100);
      const el = mockElement(document.body);

      tracker.handleScroll(term, el);

      expect(tracker.isUserScrolledUp()).toBe(false);
    });

    it("ignores scroll events when container is hidden (offsetParent null)", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(100, 100);
      const visibleEl = mockElement(document.body);
      const hiddenEl = mockElement(null);

      // User starts at bottom
      tracker.handleScroll(term, visibleEl);
      expect(tracker.isUserScrolledUp()).toBe(false);

      // Browser resets scrollTop to 0 while hidden → onScroll fires with
      // viewportY=0, baseY=100.  This should be IGNORED.
      term.buffer.active.viewportY = 0;
      tracker.handleScroll(term, hiddenEl);

      // userScrolledUp must still be false — the hidden scroll was ignored
      expect(tracker.isUserScrolledUp()).toBe(false);
    });

    it("works when containerEl is undefined", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(50, 100);

      // Should not throw, and should still update state
      tracker.handleScroll(term, undefined);
      expect(tracker.isUserScrolledUp()).toBe(true);
    });
  });

  describe("fitPreservingScroll", () => {
    it("scrolls to bottom when user was at bottom", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(100, 100);
      const fitAddon = mockFitAddon();

      // Default state: userScrolledUp = false (at bottom)
      tracker.fitPreservingScroll(term, fitAddon);

      expect(fitAddon.fit).toHaveBeenCalledOnce();
      expect(term.scrollToBottom).toHaveBeenCalledOnce();
      expect(term.scrollToLine).not.toHaveBeenCalled();
    });

    it("restores exact position when user was scrolled up and fit() resets viewport", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(50, 100);
      const el = mockElement(document.body);

      // Mark user as scrolled up
      tracker.handleScroll(term, el);
      expect(tracker.isUserScrolledUp()).toBe(true);

      // Simulate fit() resetting viewport to 0 (the bug)
      const fitAddon = mockFitAddon(() => {
        term.buffer.active.viewportY = 0;
      });

      vi.mocked(term.scrollToBottom).mockClear();
      vi.mocked(term.scrollToLine).mockClear();

      tracker.fitPreservingScroll(term, fitAddon);

      expect(fitAddon.fit).toHaveBeenCalledOnce();
      expect(term.scrollToBottom).not.toHaveBeenCalled();
      expect(term.scrollToLine).toHaveBeenCalledWith(50);
    });

    it("does not call scrollToLine when viewport position is unchanged", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(50, 100);
      const el = mockElement(document.body);

      // Mark user as scrolled up
      tracker.handleScroll(term, el);

      // fit() does NOT change viewportY this time
      const fitAddon = mockFitAddon();

      vi.mocked(term.scrollToBottom).mockClear();
      vi.mocked(term.scrollToLine).mockClear();

      tracker.fitPreservingScroll(term, fitAddon);

      expect(term.scrollToBottom).not.toHaveBeenCalled();
      // viewportY stayed at 50 → no need to restore
      expect(term.scrollToLine).not.toHaveBeenCalled();
    });

    it("correctly handles hide→show cycle: scrolls to bottom on return", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(100, 100);
      const visibleEl = mockElement(document.body);
      const hiddenEl = mockElement(null);

      // 1. User is at bottom
      tracker.handleScroll(term, visibleEl);
      expect(tracker.isUserScrolledUp()).toBe(false);

      // 2. Terminal goes hidden → browser resets scrollTop → onScroll fires
      term.buffer.active.viewportY = 0;
      tracker.handleScroll(term, hiddenEl);
      // Guard ignores this → still "at bottom"
      expect(tracker.isUserScrolledUp()).toBe(false);

      // 3. Terminal becomes visible → fit is called
      term.buffer.active.viewportY = 0; // viewport is at 0 after unhide
      const fitAddon = mockFitAddon();
      tracker.fitPreservingScroll(term, fitAddon);

      // Should scroll to bottom (user was at bottom before hide)
      expect(term.scrollToBottom).toHaveBeenCalledOnce();
      expect(term.scrollToLine).not.toHaveBeenCalled();
    });

    it("preserves scrolled-up position across hide→show cycle", () => {
      const tracker = createScrollTracker();
      const term = mockTerminal(30, 100);
      const visibleEl = mockElement(document.body);
      const hiddenEl = mockElement(null);

      // 1. User is scrolled up to line 30
      tracker.handleScroll(term, visibleEl);
      expect(tracker.isUserScrolledUp()).toBe(true);

      // 2. Terminal goes hidden → browser resets scrollTop
      term.buffer.active.viewportY = 0;
      tracker.handleScroll(term, hiddenEl);
      // Guard ignores → still "scrolled up"
      expect(tracker.isUserScrolledUp()).toBe(true);

      // 3. Terminal becomes visible → fit() resets viewport to 0
      term.buffer.active.viewportY = 30; // savedY will be 30
      const fitAddon = mockFitAddon(() => {
        term.buffer.active.viewportY = 0; // fit resets to 0
      });
      tracker.fitPreservingScroll(term, fitAddon);

      // Should restore to line 30 (not scroll to bottom)
      expect(term.scrollToBottom).not.toHaveBeenCalled();
      expect(term.scrollToLine).toHaveBeenCalledWith(30);
    });
  });
});
