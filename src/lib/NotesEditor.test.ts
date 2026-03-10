import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import { tick } from "svelte";
import NotesEditor from "./NotesEditor.svelte";
import {
  activeNote,
  focusTarget,
  hotkeyAction,
  notePreviewMode,
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
    notePreviewMode.set(false);
    focusTarget.set(null);
    hotkeyAction.set(null);
  });

  it("keeps the latest note content when read_note resolves out of order", async () => {
    const noteARequest = deferred<string>();
    const noteBRequest = deferred<string>();

    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
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
      expect(invoke).toHaveBeenCalledWith("read_note", {
        projectName: "Project Alpha",
        filename: "a.md",
      });
    });

    activeNote.set({ projectId: "project-1", filename: "b.md" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("read_note", {
        projectName: "Project Alpha",
        filename: "b.md",
      });
    });

    noteBRequest.resolve("newest note content");

    const textarea = await screen.findByRole("textbox");
    await waitFor(() => {
      expect(textarea).toHaveValue("newest note content");
    });

    noteARequest.resolve("stale note content");
    await tick();
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(textarea).toHaveValue("newest note content");
  });
});
