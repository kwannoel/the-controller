import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import SecureEnvModal from "./SecureEnvModal.svelte";

describe("SecureEnvModal", () => {
  it("renders project and key metadata with a masked input by default", () => {
    render(SecureEnvModal, {
      projectName: "demo-project",
      envKey: "OPENAI_API_KEY",
      onSubmit: vi.fn(),
      onClose: vi.fn(),
    });

    expect(screen.getByText("demo-project")).toBeInTheDocument();
    expect(screen.getByText("OPENAI_API_KEY")).toBeInTheDocument();
    expect(screen.getByLabelText("Secret value")).toHaveAttribute("type", "password");
  });

  it("submits the entered value without trimming it", async () => {
    const onSubmit = vi.fn();
    render(SecureEnvModal, {
      projectName: "demo-project",
      envKey: "OPENAI_API_KEY",
      onSubmit,
      onClose: vi.fn(),
    });

    const input = screen.getByLabelText("Secret value");
    await fireEvent.input(input, { target: { value: "  new-secret  " } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit).toHaveBeenCalledWith("  new-secret  ");
  });

  it("calls onClose when cancel is clicked", async () => {
    const onClose = vi.fn();
    render(SecureEnvModal, {
      projectName: "demo-project",
      envKey: "OPENAI_API_KEY",
      onSubmit: vi.fn(),
      onClose,
    });

    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
