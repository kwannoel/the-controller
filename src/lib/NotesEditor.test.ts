import { beforeEach, describe, expect, it, vi } from "vitest";
import { command } from "$lib/backend";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { tick } from "svelte";
import { get } from "svelte/store";
import NotesEditor from "./NotesEditor.svelte";
import {
  activeNote,
  focusTarget,
  hotkeyAction,
  projects,
  type Project,
} from "./stores";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const baseProject: Project = {
  id: "project-1",
  name: "Project Alpha",
  repo_path: "/tmp/project-alpha",
  created_at: "2026-03-10T00:00:00Z",
  archived: false,
  sessions: [],
  maintainer: { enabled: false, interval_minutes: 60 },
  auto_worker: { enabled: false },
  prompts: [],
  staged_session: null,
};

describe("NotesEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([baseProject]);
    activeNote.set({ projectId: "project-1", filename: "a.md" });
    focusTarget.set(null);
    hotkeyAction.set(null);
  });

  it("mounts a dedicated code editor surface", async () => {
    vi.mocked(command).mockImplementation((commandName: string) => {
      if (commandName === "read_note") {
        return Promise.resolve("# Heading\n\nBody copy");
      }

      if (commandName === "write_note") {
        return Promise.resolve(undefined);
      }

      return Promise.resolve(undefined);
    });

    render(NotesEditor);

    expect(await screen.findByTestId("note-code-editor")).toBeInTheDocument();
  });

  it("returns to the note on a single escape when vim is already in normal mode", async () => {
    vi.mocked(command).mockImplementation((commandName: string) => {
      if (commandName === "read_note") {
        return Promise.resolve("# Heading\n\nBody copy");
      }

      if (commandName === "write_note") {
        return Promise.resolve(undefined);
      }

      return Promise.resolve(undefined);
    });

    focusTarget.set({ type: "notes-editor", projectId: "project-1" });

    render(NotesEditor);
    const user = userEvent.setup();

    const editor = await screen.findByTestId("note-code-editor");
    const textbox = within(editor).getByRole("textbox");
    textbox.focus();

    await user.keyboard("{Escape}");

    expect(get(focusTarget)).toEqual({ type: "note", filename: "a.md", projectId: "project-1" });
  });

  it("keeps escape in the editor when leaving insert mode, then exits on the next normal-mode escape", async () => {
    vi.mocked(command).mockImplementation((commandName: string) => {
      if (commandName === "read_note") {
        return Promise.resolve("# Heading\n\nBody copy");
      }

      if (commandName === "write_note") {
        return Promise.resolve(undefined);
      }

      return Promise.resolve(undefined);
    });

    focusTarget.set({ type: "notes-editor", projectId: "project-1" });

    render(NotesEditor);
    const user = userEvent.setup();

    const editor = await screen.findByTestId("note-code-editor");
    const textbox = within(editor).getByRole("textbox");
    textbox.focus();

    await user.keyboard("i");
    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "notes-editor", projectId: "project-1" });

    await new Promise((resolve) => setTimeout(resolve, 350));

    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "note", filename: "a.md", projectId: "project-1" });
  });

  it("keeps escape in the editor when leaving visual mode, then exits on the next normal-mode escape", async () => {
    vi.mocked(command).mockImplementation((commandName: string) => {
      if (commandName === "read_note") {
        return Promise.resolve("# Heading\n\nBody copy");
      }

      if (commandName === "write_note") {
        return Promise.resolve(undefined);
      }

      return Promise.resolve(undefined);
    });

    focusTarget.set({ type: "notes-editor", projectId: "project-1" });

    render(NotesEditor);
    const user = userEvent.setup();

    const editor = await screen.findByTestId("note-code-editor");
    const textbox = within(editor).getByRole("textbox");
    textbox.focus();

    await user.keyboard("v");
    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "notes-editor", projectId: "project-1" });

    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "note", filename: "a.md", projectId: "project-1" });
  });

  it("keeps the latest note content when read_note resolves out of order", async () => {
    const noteARequest = deferred<string>();
    const noteBRequest = deferred<string>();

    vi.mocked(command).mockImplementation((command: string, args?: unknown) => {
      if (command === "read_note") {
        const filename = (args as { filename?: string } | undefined)?.filename;
        if (filename === "a.md") return noteARequest.promise;
        if (filename === "b.md") return noteBRequest.promise;
      }

      if (command === "write_note") {
        return Promise.resolve(undefined);
      }

      return Promise.resolve(undefined);
    });

    render(NotesEditor);

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("read_note", {
        projectName: "Project Alpha",
        filename: "a.md",
      });
    });

    activeNote.set({ projectId: "project-1", filename: "b.md" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("read_note", {
        projectName: "Project Alpha",
        filename: "b.md",
      });
    });

    noteBRequest.resolve("newest note content");

    await waitFor(() => {
      expect(screen.getByTestId("note-code-editor")).toHaveTextContent("newest note content");
    });

    noteARequest.resolve("stale note content");
    await tick();
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByTestId("note-code-editor")).toHaveTextContent("newest note content");
  });
});
