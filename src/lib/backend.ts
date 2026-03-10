const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;

let sharedWs: WebSocket | null = null;

function getSharedWebSocket(): WebSocket {
  if (!sharedWs || sharedWs.readyState === WebSocket.CLOSED || sharedWs.readyState === WebSocket.CLOSING) {
    const wsUrl = `ws://${window.location.hostname}:3001/ws`;
    sharedWs = new WebSocket(wsUrl);
  }
  return sharedWs;
}

export async function command<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function listen<T>(event: string, handler: (payload: T) => void): () => void {
  if (isTauri) {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<T>(event, (e) => handler(e.payload)).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }

  const ws = getSharedWebSocket();
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  ws.addEventListener("message", callback);
  return () => ws.removeEventListener("message", callback);
}
