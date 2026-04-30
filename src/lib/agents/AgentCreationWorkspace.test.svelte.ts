import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import AgentCreationWorkspace from "./AgentCreationWorkspace.svelte";
import { daemonStore } from "../daemon/store.svelte";
import type { AgentProfile, SavedAgentProfile } from "../daemon/types";

function makeProfile(overrides: Partial<AgentProfile> = {}): AgentProfile {
  return {
    id: "profile-1",
    handle: "reviewer",
    name: "Reviewer",
    description: "Reviews changes",
    runtime: "codex",
    skills: ["code-review"],
    prompt: "Review the active diff and return concise findings.",
    archived_at: null,
    avatar_asset_path: null,
    avatar_status: "pending",
    avatar_error: null,
    active_version_id: "version-1",
    created_at: 1,
    updated_at: 1,
    ...overrides,
  };
}

function makeSaved(profile: AgentProfile): SavedAgentProfile {
  return {
    profile,
    version: {
      id: "version-2",
      profile_id: profile.id,
      runtime: profile.runtime,
      model: "gpt-5",
      prompt: profile.prompt,
      skills: profile.skills,
      default_workspace_behavior: "focused",
      outbox_instructions: "Reply with a concise summary.",
      validation_result: null,
      created_at: 2,
    },
  };
}

describe("AgentCreationWorkspace", () => {
  beforeEach(() => {
    daemonStore.reachable = true;
    daemonStore.profiles.clear();
    daemonStore.client = {
      saveProfile: vi.fn(),
      archiveProfile: vi.fn(),
      restoreProfile: vi.fn(),
      testProfileInChat: vi.fn(),
    } as any;
  });

  it("shows the profile empty state", () => {
    render(AgentCreationWorkspace);

    expect(screen.getByText("No agent profiles")).toBeTruthy();
    expect(screen.getByRole("button", { name: "New Profile" })).toBeTruthy();
    expect(screen.getByText(/profiles become available as @agent and %agent in chat/i)).toBeTruthy();
  });

  it("opens a new draft from the empty state", async () => {
    render(AgentCreationWorkspace);

    await fireEvent.click(screen.getByRole("button", { name: "New Profile" }));

    expect(screen.getByLabelText("Name")).toBeTruthy();
    expect(screen.getByLabelText("Handle")).toBeTruthy();
    expect(screen.getByText("Unsaved draft")).toBeTruthy();
  });

  it("opens a new draft with n when text input is not focused", async () => {
    daemonStore.profiles.set("profile-1", makeProfile());
    render(AgentCreationWorkspace);

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", bubbles: true }));

    expect(await screen.findByText("Unsaved draft")).toBeTruthy();
    expect(screen.getByLabelText("Handle")).toHaveValue("");
  });

  it("filters active and archived profile rows into separate views", async () => {
    daemonStore.profiles.set("active-profile", makeProfile({
      id: "active-profile",
      name: "Active Reviewer",
      handle: "active-reviewer",
    }));
    daemonStore.profiles.set("archived-profile", makeProfile({
      id: "archived-profile",
      name: "Archived Reviewer",
      handle: "archived-reviewer",
      archived_at: 42,
    }));

    render(AgentCreationWorkspace);

    const list = screen.getByRole("listbox", { name: "Profiles" });
    expect(within(list).getByText("Active Reviewer")).toBeTruthy();
    expect(within(list).queryByText("Archived Reviewer")).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: "Archived" }));

    expect(within(list).queryByText("Active Reviewer")).toBeNull();
    expect(within(list).getByText("Archived Reviewer")).toBeTruthy();
  });

  it("disables save when the handle is invalid", async () => {
    render(AgentCreationWorkspace);
    await fireEvent.click(screen.getByRole("button", { name: "New Profile" }));

    await userEvent.type(screen.getByLabelText("Name"), "Reviewer");
    await userEvent.type(screen.getByLabelText("Handle"), "Reviewer!");
    await userEvent.type(screen.getByLabelText("System Prompt"), "Review the active diff.");

    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(screen.getByText("Handle can use lowercase letters, numbers, and hyphens only.")).toBeTruthy();
  });

  it("saves a valid draft through the daemon client and updates the profile store", async () => {
    const savedProfile = makeProfile();
    vi.mocked(daemonStore.client!.saveProfile).mockResolvedValueOnce(makeSaved(savedProfile));
    render(AgentCreationWorkspace);
    await fireEvent.click(screen.getByRole("button", { name: "New Profile" }));

    await userEvent.type(screen.getByLabelText("Name"), "Reviewer");
    await userEvent.type(screen.getByLabelText("Handle"), "reviewer");
    await userEvent.type(screen.getByLabelText("System Prompt"), "Review the active diff.");

    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(daemonStore.client!.saveProfile).toHaveBeenCalledWith(expect.objectContaining({
        name: "Reviewer",
        handle: "reviewer",
        runtime: "codex",
        prompt: "Review the active diff.",
      }));
      expect(daemonStore.profiles.get("profile-1")).toEqual(savedProfile);
    });
  });

  it("saves a new draft with blank optional strings without sending null values", async () => {
    const savedProfile = makeProfile();
    vi.mocked(daemonStore.client!.saveProfile).mockResolvedValueOnce(makeSaved(savedProfile));
    render(AgentCreationWorkspace);
    await fireEvent.click(screen.getByRole("button", { name: "New Profile" }));

    await userEvent.type(screen.getByLabelText("Name"), "Reviewer");
    await userEvent.type(screen.getByLabelText("Handle"), "reviewer");
    await userEvent.type(screen.getByLabelText("System Prompt"), "Review the active diff.");

    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(daemonStore.client!.saveProfile).toHaveBeenCalled());
    const request = vi.mocked(daemonStore.client!.saveProfile).mock.calls[0][0];
    expect(Object.values(request)).not.toContain(null);
    expect(request.description).toBeUndefined();
    expect(request.outbox_instructions).toBeUndefined();
  });

  it("does not send unknown active-version fields when saving an existing profile name edit", async () => {
    const profile = makeProfile();
    const savedProfile = makeProfile({ name: "Reviewer Updated" });
    daemonStore.profiles.set(profile.id, profile);
    vi.mocked(daemonStore.client!.saveProfile).mockResolvedValueOnce(makeSaved(savedProfile));
    render(AgentCreationWorkspace);

    await userEvent.clear(await screen.findByLabelText("Name"));
    await userEvent.type(screen.getByLabelText("Name"), "Reviewer Updated");
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(daemonStore.client!.saveProfile).toHaveBeenCalled());
    const request = vi.mocked(daemonStore.client!.saveProfile).mock.calls[0][0];
    expect(request).toMatchObject({
      id: profile.id,
      name: "Reviewer Updated",
    });
    expect(request).not.toHaveProperty("model");
    expect(request).not.toHaveProperty("default_workspace_behavior");
    expect(request).not.toHaveProperty("outbox_instructions");
  });
});
