import { vi } from "vitest";
import { SvelteMap } from "svelte/reactivity";

export const readEvents = vi.fn();
export const createChat = vi.fn();
export const readChatTranscript = vi.fn();
export const sendMessage = vi.fn(async () => ({ seq: 1 }));
export const sendChatMessage = vi.fn();
export const bootstrap = vi.fn(async () => {});
export const pingDaemon = vi.fn(async () => {});
export const loadSessions = vi.fn(async () => {});
export const loadProfiles = vi.fn(async () => {});
export const loadChats = vi.fn(async () => {});

export const daemonStore = $state<any>({
  client: {
    readEvents,
    readChatTranscript,
    createChat,
    sendMessage,
    sendChatMessage,
    wsUrl: () => "ws://x",
    chatStreamUrl: () => "ws://chat",
  },
  sessions: new SvelteMap([["s1", { id: "s1", label: "Chat 1", agent: "claude", status: "running" }]]),
  transcripts: new SvelteMap(),
  activeSessionId: "s1",
  profiles: new SvelteMap(),
  chats: new SvelteMap(),
  chatTranscripts: new SvelteMap(),
  activeChatId: null,
  reachable: true,
  newChatTarget: null,
});
