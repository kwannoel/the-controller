import { SvelteMap } from "svelte/reactivity";
import { DaemonClient } from "./client";
import type { AgentProfile, AgentTurnTrace, Chat, ChatMetrics, ChatTranscriptEntry, DaemonSession } from "./types";
import { emptyTranscript, type TranscriptState } from "./reducer";

const BASE_URL = "/api/daemon";

interface StoreState {
  reachable: boolean;
  client: DaemonClient | null;
  sessions: SvelteMap<string, DaemonSession>;
  transcripts: SvelteMap<string, TranscriptState>;
  activeSessionId: string | null;
  profiles: SvelteMap<string, AgentProfile>;
  chats: SvelteMap<string, Chat>;
  activeChatId: string | null;
  chatTranscripts: SvelteMap<string, ChatTranscriptEntry[]>;
  chatSummaries: SvelteMap<string, ChatMetrics>;
  agentTraces: SvelteMap<string, AgentTurnTrace[]>;
  newChatTarget: { projectId: string; projectCwd: string } | null;
}

export const daemonStore = $state<StoreState>({
  reachable: false,
  client: null,
  sessions: new SvelteMap<string, DaemonSession>(),
  transcripts: new SvelteMap<string, TranscriptState>(),
  activeSessionId: null,
  profiles: new SvelteMap<string, AgentProfile>(),
  chats: new SvelteMap<string, Chat>(),
  activeChatId: null,
  chatTranscripts: new SvelteMap<string, ChatTranscriptEntry[]>(),
  chatSummaries: new SvelteMap<string, ChatMetrics>(),
  agentTraces: new SvelteMap<string, AgentTurnTrace[]>(),
  newChatTarget: null,
});

export async function bootstrap(): Promise<void> {
  daemonStore.client = new DaemonClient(BASE_URL);
  await pingDaemon();
}

export async function pingDaemon(): Promise<void> {
  if (!daemonStore.client) {
    daemonStore.reachable = false;
    return;
  }
  try {
    await daemonStore.client.listSessions();
    daemonStore.reachable = true;
  } catch {
    daemonStore.reachable = false;
  }
}

export async function loadSessions(): Promise<void> {
  if (!daemonStore.client) return;
  const list = await daemonStore.client.listSessions();
  // Replace the map contents in-place rather than assigning a new SvelteMap
  // (because we want the subscribers to keep tracking the same reactive instance).
  daemonStore.sessions.clear();
  for (const s of list) {
    daemonStore.sessions.set(s.id, s);
    if (!daemonStore.transcripts.has(s.id)) {
      daemonStore.transcripts.set(s.id, emptyTranscript());
    }
  }
}

export async function loadProfiles(): Promise<void> {
  if (!daemonStore.client) return;
  const list = await daemonStore.client.listProfiles();
  daemonStore.profiles.clear();
  for (const profile of list) {
    daemonStore.profiles.set(profile.id, profile);
  }
}

export async function loadChats(): Promise<void> {
  if (!daemonStore.client) return;
  const list = await daemonStore.client.listChats();
  daemonStore.chats.clear();
  for (const chat of list) {
    daemonStore.chats.set(chat.id, chat);
    if (!daemonStore.chatTranscripts.has(chat.id)) {
      daemonStore.chatTranscripts.set(chat.id, []);
    }
  }
}

export async function loadChatTranscript(chatId: string): Promise<void> {
  if (!daemonStore.client) return;
  const transcript = await daemonStore.client.readChatTranscript(chatId);
  daemonStore.chatTranscripts.set(chatId, transcript);
}

export async function loadAgentTrace(sessionId: string): Promise<void> {
  if (!daemonStore.client) return;
  const trace = await daemonStore.client.getAgentTrace(sessionId);
  daemonStore.agentTraces.set(sessionId, trace);
}

export async function loadChatMetrics(chatId: string): Promise<void> {
  if (!daemonStore.client) return;
  const metrics = await daemonStore.client.getChatMetrics(chatId);
  daemonStore.chatSummaries.set(chatId, metrics);
}
