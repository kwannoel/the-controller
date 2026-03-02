import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { toasts, showToast } from './toast';

describe('toast', () => {
  beforeEach(() => {
    toasts.set([]);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('showToast adds a toast with correct text and type', () => {
    showToast('Something went wrong', 'error');
    const list = get(toasts);
    expect(list).toHaveLength(1);
    expect(list[0].text).toBe('Something went wrong');
    expect(list[0].type).toBe('error');
  });

  it('default type is info', () => {
    showToast('hello');
    expect(get(toasts)[0].type).toBe('info');
  });

  it('auto-removes after 5 seconds', () => {
    showToast('temporary');
    expect(get(toasts)).toHaveLength(1);
    vi.advanceTimersByTime(5000);
    expect(get(toasts)).toHaveLength(0);
  });

  it('multiple toasts get unique IDs', () => {
    showToast('first');
    showToast('second');
    const list = get(toasts);
    expect(list).toHaveLength(2);
    expect(list[0].id).not.toBe(list[1].id);
  });

  it('removes only the expired toast', () => {
    showToast('first');
    vi.advanceTimersByTime(2000);
    showToast('second');
    vi.advanceTimersByTime(3000); // 5s for first, 3s for second
    const list = get(toasts);
    expect(list).toHaveLength(1);
    expect(list[0].text).toBe('second');
  });
});
