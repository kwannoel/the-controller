import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { command } from '$lib/backend';
import FuzzyFinder from './FuzzyFinder.svelte';

const mockEntries = [
  { name: 'alpha-project', path: '/home/user/projects/alpha-project' },
  { name: 'beta-app', path: '/home/user/projects/beta-app' },
  { name: 'gamma-lib', path: '/home/user/projects/gamma-lib' },
];

const mockProjects = [
  { id: 'p1', name: 'existing-proj', repo_path: '/home/user/projects/existing-proj', created_at: '', archived: false, sessions: [], maintainer: { enabled: false, interval_minutes: 5 }, auto_worker: { enabled: false }, prompts: [], staged_sessions: [] },
];

describe('FuzzyFinder', () => {
  const onSelect = vi.fn();
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(command).mockResolvedValue(mockEntries);
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

    expect(screen.getByText('No matching projects')).toBeInTheDocument();
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
      expect(command).toHaveBeenCalledWith('list_root_directories');
    });
  });

  it('shows existing projects before directory entries', async () => {
    render(FuzzyFinder, { props: { projects: mockProjects as any, onSelect, onClose } });
    await screen.findByText('alpha-project');

    // Existing project should be present
    expect(screen.getByText('existing-proj')).toBeInTheDocument();
    expect(screen.getByText('loaded')).toBeInTheDocument();

    // All items rendered: 1 project + 3 dirs
    const items = screen.getAllByRole('option');
    expect(items).toHaveLength(4);

    // Existing project is first
    expect(items[0]).toHaveTextContent('existing-proj');
  });

  it('deduplicates directories that match existing project repo_path', async () => {
    const projectWithMatchingDir = [
      { id: 'p2', name: 'alpha-project', repo_path: '/home/user/projects/alpha-project', created_at: '', archived: false, sessions: [], maintainer: { enabled: false, interval_minutes: 5 }, auto_worker: { enabled: false }, prompts: [], staged_sessions: [] },
    ];

    render(FuzzyFinder, { props: { projects: projectWithMatchingDir as any, onSelect, onClose } });
    await screen.findByText('beta-app');

    // alpha-project should appear once (as existing project), not twice
    const alphaElements = screen.getAllByText('alpha-project');
    expect(alphaElements).toHaveLength(1);

    // Total: 1 project + 2 remaining dirs = 3
    const items = screen.getAllByRole('option');
    expect(items).toHaveLength(3);
  });

  it('selecting an existing project includes projectId', async () => {
    const user = userEvent.setup();
    render(FuzzyFinder, { props: { projects: mockProjects as any, onSelect, onClose } });
    await screen.findByText('existing-proj');

    const input = screen.getByPlaceholderText('Search projects...');
    await user.click(input);
    await user.keyboard('{Enter}');

    expect(onSelect).toHaveBeenCalledWith({
      name: 'existing-proj',
      path: '/home/user/projects/existing-proj',
      projectId: 'p1',
    });
  });
});
