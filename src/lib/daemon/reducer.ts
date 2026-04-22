import type { EventRecord, StatusState } from "./types";

export interface TokenUsage {
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
}

export interface TranscriptState {
  events: EventRecord[];
  lastSeq: number;
  inProgressBlocks: Map<string, string>;
  statusState: StatusState | null;
  tokenUsage: TokenUsage | null;
  seenSeq: Set<number>;
}

export function emptyTranscript(): TranscriptState {
  return {
    events: [],
    lastSeq: 0,
    inProgressBlocks: new Map(),
    statusState: null,
    tokenUsage: null,
    seenSeq: new Set(),
  };
}

export function reduceTranscript(prev: TranscriptState, e: EventRecord): TranscriptState {
  if (prev.seenSeq.has(e.seq)) return prev;

  const seenSeq = new Set(prev.seenSeq);
  seenSeq.add(e.seq);
  const lastSeq = Math.max(prev.lastSeq, e.seq);

  let inProgressBlocks = prev.inProgressBlocks;
  let statusState = prev.statusState;
  let tokenUsage = prev.tokenUsage;
  let events = prev.events;

  if (e.channel === "outbox" && e.kind === "agent_text_delta") {
    const p = e.payload as { block_id: string; delta: string };
    inProgressBlocks = new Map(inProgressBlocks);
    inProgressBlocks.set(p.block_id, (inProgressBlocks.get(p.block_id) ?? "") + p.delta);
    return { ...prev, seenSeq, lastSeq, inProgressBlocks };
  }

  if (e.channel === "outbox" && e.kind === "agent_text") {
    const p = e.payload as { block_id: string };
    if (inProgressBlocks.has(p.block_id)) {
      inProgressBlocks = new Map(inProgressBlocks);
      inProgressBlocks.delete(p.block_id);
    }
    events = [...events, e];
    return { ...prev, seenSeq, lastSeq, inProgressBlocks, events };
  }

  if (e.channel === "outbox" && e.kind === "token_usage") {
    tokenUsage = e.payload as TokenUsage;
  }

  if (e.channel === "system" && e.kind === "status_changed") {
    statusState = (e.payload as { state: StatusState }).state;
  }

  events = [...events, e];
  return { ...prev, seenSeq, lastSeq, events, inProgressBlocks, statusState, tokenUsage };
}
