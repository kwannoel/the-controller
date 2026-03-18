type SessionKind = "claude" | "codex" | "cursor-agent" | undefined;

type InvokeFn = (
  command: string,
  args: { sessionId: string; data: string },
) => Promise<unknown>;

export async function sendFinishBranchPrompt(
  invoke: InvokeFn,
  sessionId: string,
  kind: SessionKind,
) {
  const isCodex = kind === "codex";
  const prompt = isCodex
    ? "$finishing-a-development-branch"
    : "/finishing-a-development-branch";

  if (isCodex) {
    await invoke("write_to_pty", { sessionId, data: prompt });
    await invoke("send_raw_to_pty", { sessionId, data: "\r" });
    return;
  }

  await invoke("write_to_pty", { sessionId, data: `${prompt}\r` });
}
