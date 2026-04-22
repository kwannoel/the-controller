import { command } from "$lib/backend";
import { SvelteMap } from "svelte/reactivity";
import { DaemonClient } from "./client";
import type { DaemonSession } from "./types";
import { emptyTranscript, type TranscriptState } from "./reducer";

const BASE_URL = "http://127.0.0.1:4867";

interface StoreState {
  token: string | null;
  reachable: boolean;
  client: DaemonClient | null;
  sessions: SvelteMap<string, DaemonSession>;
  transcripts: SvelteMap<string, TranscriptState>;
  activeSessionId: string | null;
  newChatTarget: { projectId: string; projectCwd: string } | null;
}

export const daemonStore = $state<StoreState>({
  token: null,
  reachable: false,
  client: null,
  sessions: new SvelteMap<string, DaemonSession>(),
  transcripts: new SvelteMap<string, TranscriptState>(),
  activeSessionId: null,
  newChatTarget: null,
});

export async function bootstrap(): Promise<void> {
  try {
    const token = await command<string>("read_daemon_token");
    daemonStore.token = token;
    daemonStore.client = new DaemonClient(BASE_URL, token);
  } catch (e) {
    daemonStore.reachable = false;
    daemonStore.token = null;
    daemonStore.client = null;
    return;
  }
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
