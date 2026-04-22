import { daemonStore } from "./store.svelte";
import { reduceTranscript, emptyTranscript } from "./reducer";
import type { EventRecord } from "./types";

export interface StreamHandle {
  close(): void;
}

export function openStream(sessionId: string): StreamHandle {
  let closed = false;
  let ws: WebSocket | null = null;
  let attempt = 0;

  function connect() {
    if (closed) return;
    const client = daemonStore.client;
    if (!client) return;
    const t = daemonStore.transcripts.get(sessionId);
    const since = t?.lastSeq ?? 0;
    const url = client.wsUrl(sessionId, since);
    ws = new WebSocket(url);
    ws.onmessage = (ev) => {
      try {
        const evt = JSON.parse(ev.data) as EventRecord;
        const prev = daemonStore.transcripts.get(sessionId) ?? emptyTranscript();
        const next = reduceTranscript(prev, evt);
        daemonStore.transcripts.set(sessionId, next);
      } catch {
        // swallow parse errors; the daemon emits valid JSON
      }
    };
    ws.onclose = () => {
      if (closed) return;
      const delay = Math.min(10_000, 500 * 2 ** attempt);
      attempt += 1;
      setTimeout(connect, delay);
    };
    ws.onopen = () => { attempt = 0; };
  }

  connect();
  return {
    close() {
      closed = true;
      ws?.close();
    },
  };
}
