import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import AgentTokenMenu from "./AgentTokenMenu.svelte";
import type { AgentProfile } from "../daemon/types";

function makeProfile(overrides: Partial<AgentProfile> = {}): AgentProfile {
  return {
    id: "profile-1",
    handle: "reviewer",
    name: "Reviewer",
    description: "Reviews changes",
    runtime: "codex",
    skills: [],
    prompt: "Review changes.",
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

describe("AgentTokenMenu", () => {
  it("selects a shadow route token for an active matching profile", async () => {
    const onSelect = vi.fn();
    render(AgentTokenMenu, {
      kind: "shadow",
      query: "rev",
      profiles: [makeProfile()],
      onSelect,
    });

    await fireEvent.click(screen.getByRole("option", { name: /%reviewer/i }));

    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({
      kind: "shadow",
      profileId: "profile-1",
      handle: "reviewer",
    }));
  });

  it("filters archived profiles and matches by name", () => {
    render(AgentTokenMenu, {
      kind: "reusable",
      query: "design",
      profiles: [
        makeProfile({ id: "profile-1", handle: "reviewer", name: "Reviewer" }),
        makeProfile({ id: "profile-2", handle: "designer", name: "Design Partner" }),
        makeProfile({ id: "profile-3", handle: "design-archive", name: "Design Archive", archived_at: 10 }),
      ],
      onSelect: vi.fn(),
    });

    expect(screen.getByRole("option", { name: /@designer/i })).toBeTruthy();
    expect(screen.queryByRole("option", { name: /@design-archive/i })).toBeNull();
  });

  it("supports keyboard selection and escape close", async () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    render(AgentTokenMenu, {
      kind: "reusable",
      query: "",
      profiles: [
        makeProfile({ id: "profile-1", handle: "reviewer", name: "Reviewer" }),
        makeProfile({ id: "profile-2", handle: "zdebugger", name: "Debugger" }),
      ],
      onSelect,
      onClose,
    });

    const listbox = screen.getByRole("listbox", { name: "Agent token suggestions" });
    await fireEvent.keyDown(listbox, { key: "ArrowDown" });
    await fireEvent.keyDown(listbox, { key: "Enter" });

    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({
      kind: "reusable",
      profileId: "profile-2",
      handle: "zdebugger",
    }));

    await fireEvent.keyDown(listbox, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });
});
