import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import DeploySetupModal from "./DeploySetupModal.svelte";

vi.mock("$lib/backend", () => ({
  command: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockReturnValue(vi.fn()),
}));

describe("DeploySetupModal", () => {
  it("renders step 1 with Hetzner API key input", () => {
    render(DeploySetupModal, { onComplete: vi.fn(), onClose: vi.fn() });
    expect(screen.getByText("Hetzner API Key")).toBeTruthy();
  });

  it("calls onClose when cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(DeploySetupModal, { onComplete: vi.fn(), onClose });
    const closeBtn = screen.getByRole("button", { name: /cancel/i });
    await fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });
});
