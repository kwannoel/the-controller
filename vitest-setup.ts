import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

vi.mock('$lib/backend', () => ({
  command: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn(() => () => {}),
  listenAsync: vi.fn(async () => () => {}),
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

if (typeof Range !== 'undefined' && !Range.prototype.getClientRects) {
  Range.prototype.getClientRects = function getClientRects() {
    return {
      length: 0,
      item: () => null,
      [Symbol.iterator]: function* emptyIterator() {},
    } as DOMRectList;
  };
}
