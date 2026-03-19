/**
 * Terminal scroll-position preservation utilities.
 *
 * When xterm.js resizes (via FitAddon.fit()), the underlying
 * terminal.resize() can reset the viewport scroll position to the top
 * during buffer reflow.  These helpers track the user's scroll state and
 * restore it after a fit() call.
 */

/** Minimal subset of xterm.js Terminal used by scroll helpers. */
export interface ScrollableTerminal {
  buffer: { active: { viewportY: number; baseY: number } };
  scrollToBottom(): void;
  scrollToLine(line: number): void;
}

/** Minimal subset of FitAddon used by scroll helpers. */
export interface Fittable {
  fit(): void;
}

/**
 * Create a scroll-position tracker for a terminal.
 *
 * Returns an object with:
 * - `handleScroll(containerEl)` — call from `term.onScroll()`.  Pass the
 *   container element so we can ignore scroll events fired while the
 *   terminal is hidden (`display: none` resets `scrollTop` to 0 which
 *   would otherwise corrupt the `userScrolledUp` flag).
 * - `fitPreservingScroll(term, fitAddon)` — call instead of bare
 *   `fitAddon.fit()` to save/restore the viewport position.
 * - `isUserScrolledUp()` — read-only accessor for tests.
 */
export function createScrollTracker() {
  let userScrolledUp = false;

  return {
    isUserScrolledUp: () => userScrolledUp,

    /**
     * Update `userScrolledUp` from a terminal scroll event.
     * Ignores events when the terminal is hidden to prevent the browser's
     * `scrollTop` reset (on `display: none`) from corrupting our state.
     */
    handleScroll(term: ScrollableTerminal, containerEl: HTMLElement | undefined) {
      // When the terminal is hidden (display:none ancestor), the browser
      // resets the viewport scrollTop to 0.  xterm.js still fires onScroll
      // for that reset — we must ignore it so userScrolledUp isn't
      // incorrectly set to true.
      if (containerEl && containerEl.offsetParent === null) return;

      const buf = term.buffer.active;
      userScrolledUp = buf.viewportY < buf.baseY;
    },

    /**
     * Call `fitAddon.fit()` while preserving the user's scroll position.
     * If the user was at the bottom, keeps them there.  If scrolled up,
     * restores the exact viewport line.
     */
    fitPreservingScroll(term: ScrollableTerminal, fitAddon: Fittable) {
      const wasAtBottom = !userScrolledUp;
      const savedY = term.buffer.active.viewportY;

      fitAddon.fit();

      if (wasAtBottom) {
        term.scrollToBottom();
      } else if (term.buffer.active.viewportY !== savedY) {
        term.scrollToLine(savedY);
      }
    },
  };
}
