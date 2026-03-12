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
  noteViewMode,
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

describe("NotesEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    activeNote.set({ folder: "Project Alpha", filename: "a.md" });
    noteViewMode.set("edit");
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

    focusTarget.set({ type: "notes-editor", folder: "Project Alpha" });

    render(NotesEditor);
    const user = userEvent.setup();

    const editor = await screen.findByTestId("note-code-editor");
    const textbox = within(editor).getByRole("textbox");
    textbox.focus();

    await user.keyboard("{Escape}");

    expect(get(focusTarget)).toEqual({ type: "note", filename: "a.md", folder: "Project Alpha" });
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

    focusTarget.set({ type: "notes-editor", folder: "Project Alpha" });

    render(NotesEditor);
    const user = userEvent.setup();

    const editor = await screen.findByTestId("note-code-editor");
    const textbox = within(editor).getByRole("textbox");
    textbox.focus();

    await user.keyboard("i");
    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "notes-editor", folder: "Project Alpha" });

    await new Promise((resolve) => setTimeout(resolve, 350));

    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "note", filename: "a.md", folder: "Project Alpha" });
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

    focusTarget.set({ type: "notes-editor", folder: "Project Alpha" });

    render(NotesEditor);
    const user = userEvent.setup();

    const editor = await screen.findByTestId("note-code-editor");
    const textbox = within(editor).getByRole("textbox");
    textbox.focus();

    await user.keyboard("v");
    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "notes-editor", folder: "Project Alpha" });

    await user.keyboard("{Escape}");
    expect(get(focusTarget)).toEqual({ type: "note", filename: "a.md", folder: "Project Alpha" });
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
        folder: "Project Alpha",
        filename: "a.md",
      });
    });

    activeNote.set({ folder: "Project Alpha", filename: "b.md" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("read_note", {
        folder: "Project Alpha",
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
