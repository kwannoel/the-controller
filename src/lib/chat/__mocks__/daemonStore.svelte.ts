import { vi } from "vitest";
import { SvelteMap } from "svelte/reactivity";

export const readEvents = vi.fn();

export const daemonStore = $state<any>({
  client: { readEvents, wsUrl: () => "ws://x" },
  sessions: new SvelteMap([["s1", { id: "s1", label: "Chat 1", agent: "claude", status: "running" }]]),
  transcripts: new SvelteMap(),
  activeSessionId: "s1",
  reachable: true,
  newChatTarget: null,
});
