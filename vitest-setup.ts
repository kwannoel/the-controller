import '@testing-library/jest-dom/vitest';
import { vi, beforeEach } from 'vitest';
import { writable } from 'svelte/store';

const mockAuthError = writable(false);

vi.mock('$lib/backend', () => ({
  command: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn(() => () => {}),
  authError: mockAuthError,
}));

beforeEach(() => {
  mockAuthError.set(false);
});

if (!Range.prototype.getClientRects) {
  Range.prototype.getClientRects = function getClientRects() {
    return {
      length: 0,
      item: () => null,
      [Symbol.iterator]: function* emptyIterator() {},
    } as DOMRectList;
  };
}
