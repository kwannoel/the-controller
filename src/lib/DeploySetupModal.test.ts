import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { command } from "$lib/backend";
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

  it("saves all fields required for deploy provisioning", async () => {
    const onComplete = vi.fn();
    render(DeploySetupModal, { onComplete, onClose: vi.fn() });

    await fireEvent.input(screen.getByLabelText("Hetzner API Key"), {
      target: { value: "hetzner-token" },
    });
    await fireEvent.click(screen.getByRole("button", { name: /^next$/i }));

    await fireEvent.input(screen.getByLabelText("Cloudflare API Key"), {
      target: { value: "cloudflare-token" },
    });
    await fireEvent.input(screen.getByLabelText("Root Domain"), {
      target: { value: "example.com" },
    });
    await fireEvent.click(screen.getByRole("button", { name: /^next$/i }));

    await fireEvent.input(screen.getByLabelText("Coolify URL"), {
      target: { value: "https://coolify.example.com" },
    });
    await fireEvent.input(screen.getByLabelText("Coolify API Key"), {
      target: { value: "coolify-token" },
    });
    await fireEvent.input(screen.getByLabelText("Server IP"), {
      target: { value: "203.0.113.42" },
    });
    await fireEvent.click(screen.getByRole("button", { name: /finish/i }));

    expect(command).toHaveBeenCalledWith("save_deploy_credentials", {
      credentials: {
        hetzner_api_key: "hetzner-token",
        cloudflare_api_key: "cloudflare-token",
        cloudflare_zone_id: null,
        root_domain: "example.com",
        coolify_url: "https://coolify.example.com",
        coolify_api_key: "coolify-token",
        server_ip: "203.0.113.42",
      },
    });
    expect(onComplete).toHaveBeenCalled();
  });
});
