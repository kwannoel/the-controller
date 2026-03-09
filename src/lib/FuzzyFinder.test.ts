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

  it('calls onClose on Escape in search mode', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Escape}');

    expect(onClose).toHaveBeenCalled();
  });

  it('Enter enters navigate mode instead of selecting', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Enter}');

    expect(onSelect).not.toHaveBeenCalled();
    expect(input).toHaveAttribute('readonly');
  });

  it('j/k navigate and l selects in navigate mode', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    // Enter nav mode
    await user.keyboard('{Enter}');
    // j moves down to beta-app
    await user.keyboard('j');
    // l selects it
    await user.keyboard('l');

    expect(onSelect).toHaveBeenCalledWith(mockEntries[1]);
  });

  it('k moves up in navigate mode', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    // Enter nav mode, move down twice, then up once
    await user.keyboard('{Enter}');
    await user.keyboard('j');
    await user.keyboard('j');
    await user.keyboard('k');
    await user.keyboard('l');

    expect(onSelect).toHaveBeenCalledWith(mockEntries[1]);
  });

  it('Escape in navigate mode returns to search mode', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Enter}');
    expect(input).toHaveAttribute('readonly');

    await user.keyboard('{Escape}');
    expect(input).not.toHaveAttribute('readonly');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('typing in navigate mode returns to search mode', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await screen.findByText('alpha-project');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Enter}');
    expect(input).toHaveAttribute('readonly');

    // Typing a non-nav character returns to search mode
    await user.keyboard('a');
    expect(input).not.toHaveAttribute('readonly');
  });

  it('invokes list_root_directories on mount', async () => {
    render(FuzzyFinder, { props: { onSelect, onClose } });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('list_root_directories');
    });
  });
});
