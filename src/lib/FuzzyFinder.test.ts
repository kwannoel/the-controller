import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import FuzzyFinder from './FuzzyFinder.svelte';

const mockEntries = [
  { name: 'alpha-project', path: '/home/user/projects/alpha-project' },
  { name: 'beta-app', path: '/home/user/projects/beta-app' },
  { name: 'gamma-lib', path: '/home/user/projects/gamma-lib' },
];

describe('FuzzyFinder', () => {
  const onSelect = vi.fn();
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(mockEntries);
  });

  it('renders search input', () => {
    render(FuzzyFinder, { props: { onSelect, onClose } });
    expect(screen.getByPlaceholderText('Search projects...')).toBeInTheDocument();
  });

  it('loads and displays directory entries', async () => {
    render(FuzzyFinder, { props: { onSelect, onClose } });
    expect(await screen.findByText('alpha-project')).toBeInTheDocument();
    expect(screen.getByText('beta-app')).toBeInTheDocument();
    expect(screen.getByText('gamma-lib')).toBeInTheDocument();
  });

  it('filters entries by query', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.type(input, 'beta');

    expect(screen.getByText('beta-app')).toBeInTheDocument();
    expect(screen.queryByText('alpha-project')).not.toBeInTheDocument();
  });

  it('shows empty state when no matches', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.type(input, 'nonexistent');

    expect(screen.getByText('No matching directories')).toBeInTheDocument();
  });

  it('calls onClose on Escape', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Escape}');

    expect(onClose).toHaveBeenCalled();
  });

  it('Enter selects the highlighted item', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Enter}');

    expect(onSelect).toHaveBeenCalledWith(mockEntries[0]);
  });

  it('ArrowDown + Enter selects the second item', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{ArrowDown}');
    await user.keyboard('{Enter}');

    expect(onSelect).toHaveBeenCalledWith(mockEntries[1]);
  });

  it('ArrowUp moves selection up', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{ArrowDown}');
    await user.keyboard('{ArrowDown}');
    await user.keyboard('{ArrowUp}');
    await user.keyboard('{Enter}');

    expect(onSelect).toHaveBeenCalledWith(mockEntries[1]);
  });

  it('invokes list_root_directories on mount', async () => {
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('list_root_directories');
    });
  });
});
