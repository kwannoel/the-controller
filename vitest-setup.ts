import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

vi.mock('$lib/backend', () => ({
  command: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn(() => () => {}),
}));
