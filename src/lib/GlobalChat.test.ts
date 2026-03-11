import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { command, listen } from "$lib/backend";
import { get } from "svelte/store";
import GlobalChat from "./GlobalChat.svelte";
import {
  activeNote,
  controllerChatSession,
  focusTarget,
  noteEntries,
  projects,
  workspaceMode,
  type Project,
} from "./stores";

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

describe("GlobalChat", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([baseProject]);
    focusTarget.set({ type: "project", projectId: "project-1" });
    workspaceMode.set("development");
    activeNote.set(null);
    noteEntries.set(new Map());
    controllerChatSession.set({
      focus: {
        project_id: "project-1",
        project_name: "Project Alpha",
        session_id: null,
        note_filename: null,
        workspace_mode: "development",
      },
      items: [],
      turn_in_progress: false,
    });
  });

  it("renders controller bridge activity rows after a chat turn", async () => {
    controllerChatSession.set({
      focus: {
        project_id: "project-1",
        project_name: "Project Alpha",
        session_id: null,
        note_filename: "issue-123.md",
        workspace_mode: "notes",
      },
      items: [
        { kind: "user", text: "fetch issue 123" },
        { kind: "tool", text: "controller.create_note(issue-123.md)" },
        { kind: "assistant", text: "Created the note and opened it." },
      ],
      turn_in_progress: false,
    });

    vi.mocked(command).mockResolvedValue(get(controllerChatSession));

    render(GlobalChat);

    expect(await screen.findByText("controller.create_note(issue-123.md)")).toBeInTheDocument();
    expect(screen.getByText("Created the note and opened it.")).toBeInTheDocument();
    expect(screen.getByTestId("controller-chat-focus")).toHaveTextContent("Project Alpha / issue-123.md");
  });

  it("submits a user message through send_controller_chat_message", async () => {
    vi.mocked(command).mockImplementation(async (commandName: string, args?: Record<string, unknown>) => {
      if (commandName === "get_controller_chat_session") {
        return get(controllerChatSession);
      }

      if (commandName === "update_controller_chat_focus") {
        return get(controllerChatSession);
      }

      if (commandName === "send_controller_chat_message") {
        expect(args).toEqual({ message: "fetch issue 123" });
        return {
          focus: {
            project_id: "project-1",
            project_name: "Project Alpha",
            session_id: null,
            note_filename: "issue-123.md",
            workspace_mode: "notes",
          },
          items: [
            { kind: "user", text: "fetch issue 123" },
            { kind: "assistant", text: "Fetched it." },
          ],
          turn_in_progress: false,
        };
      }

      return undefined;
    });

    render(GlobalChat);

    const input = screen.getByTestId("controller-chat-input");
    await fireEvent.input(input, {
      target: { value: "fetch issue 123" },
    });
    await fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("send_controller_chat_message", {
        message: "fetch issue 123",
      });
    });
    expect(get(controllerChatSession).items.at(-1)?.text).toBe("Fetched it.");
  });

  it("reacts to note-open events by updating the notes workspace stores", async () => {
    const handlers = new Map<string, (payload: string) => void>();

    vi.mocked(command).mockImplementation(async (commandName: string) => {
      if (commandName === "get_controller_chat_session") {
        return get(controllerChatSession);
      }

      if (commandName === "update_controller_chat_focus") {
        return get(controllerChatSession);
      }

      if (commandName === "list_notes") {
        return [{ filename: "issue-123.md", modified_at: "2026-03-10T00:00:00Z" }];
      }

      return undefined;
    });

    vi.mocked(listen).mockImplementation((event: string, handler: (payload: string) => void) => {
      handlers.set(event, handler);
      return () => {
        handlers.delete(event);
      };
    });

    render(GlobalChat);

    handlers.get("controller-chat-note-opened")?.(
      JSON.stringify({ project_id: "project-1", filename: "issue-123.md" }),
    );

    await waitFor(() => {
      expect(get(activeNote)).toEqual({ projectId: "project-1", filename: "issue-123.md" });
    });
    expect(get(focusTarget)).toEqual({ type: "notes-editor", projectId: "project-1" });
    expect(get(workspaceMode)).toBe("notes");
    expect(get(noteEntries).get("project-1")?.[0]?.filename).toBe("issue-123.md");
  });
});
