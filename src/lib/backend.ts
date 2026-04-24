let sharedWs: WebSocket | null = null;

function getSharedWebSocket(): WebSocket {
  if (!sharedWs || sharedWs.readyState === WebSocket.CLOSED || sharedWs.readyState === WebSocket.CLOSING) {
    const wsUrl = `ws://${window.location.host}/ws`;
    sharedWs = new WebSocket(wsUrl);
  }
  return sharedWs;
}

export async function openUrl(url: string): Promise<void> {
  window.open(url, "_blank", "noopener,noreferrer");
}

export async function command<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function listen<T>(event: string, handler: (payload: T) => void): () => void {
  const ws = getSharedWebSocket();
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  ws.addEventListener("message", callback);
  return () => ws.removeEventListener("message", callback);
}

/**
 * Like listen(), but returns a Promise that resolves once the listener is
 * actually registered. Use this when you need to guarantee the listener is
 * active before triggering events (e.g., starting a pipeline).
 */
export async function listenAsync<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  const ws = getSharedWebSocket();
  if (ws.readyState === WebSocket.CONNECTING) {
    await new Promise<void>((resolve, reject) => {
      ws.addEventListener("open", () => resolve(), { once: true });
      ws.addEventListener("error", () => reject(new Error("WebSocket connection failed")), { once: true });
    });
  }
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  ws.addEventListener("message", callback);
  return () => ws.removeEventListener("message", callback);
}
