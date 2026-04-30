import { SvelteMap } from "svelte/reactivity";
import { DaemonClient } from "./client";
import type { DaemonSession } from "./types";
import { emptyTranscript, type TranscriptState } from "./reducer";

const BASE_URL = "/api/daemon";

interface StoreState {
  reachable: boolean;
  client: DaemonClient | null;
  sessions: SvelteMap<string, DaemonSession>;
  transcripts: SvelteMap<string, TranscriptState>;
  activeSessionId: string | null;
  newChatTarget: { projectId: string; projectCwd: string } | null;
}

export const daemonStore = $state<StoreState>({
  reachable: false,
  client: null,
  sessions: new SvelteMap<string, DaemonSession>(),
  transcripts: new SvelteMap<string, TranscriptState>(),
  activeSessionId: null,
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
