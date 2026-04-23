type SessionKind = "claude" | "codex" | undefined;

type CommandFn = (
  command: string,
  args: { sessionId: string; data: string },
) => Promise<unknown>;

export async function sendFinishBranchPrompt(
  command: CommandFn,
  sessionId: string,
  kind: SessionKind,
) {
  const isCodex = kind === "codex";
  const prompt = isCodex
    ? "$the-controller-finishing-a-development-branch"
    : "/the-controller-finishing-a-development-branch";

  if (isCodex) {
    await command("write_to_pty", { sessionId, data: prompt });
    await command("send_raw_to_pty", { sessionId, data: "\r" });
    return;
  }

  await command("write_to_pty", { sessionId, data: `${prompt}\r` });
}
