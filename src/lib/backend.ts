import { get, writable } from "svelte/store";

const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;

/** True when the server rejects our auth token (401). */
export const authError = writable(false);

function getAuthToken(): string | null {
  if (isTauri) return null;
  if (typeof window === "undefined" || typeof sessionStorage === "undefined") return null;
  // Check sessionStorage first (token is moved here after initial URL read)
  const stored = sessionStorage.getItem("authToken");
  if (stored) return stored;
  const params = new URLSearchParams(window.location.search);
  const token = params.get("token") || null;
  if (token) {
    sessionStorage.setItem("authToken", token);
    // Strip token from URL to avoid leaking via history/referrer
    params.delete("token");
    const qs = params.toString();
    const newUrl = window.location.pathname + (qs ? `?${qs}` : "") + window.location.hash;
    history.replaceState(null, "", newUrl);
  }
  return token;
}

const authToken = getAuthToken();

let sharedWs: WebSocket | null = null;
let wsListeners: Array<(msg: MessageEvent) => void> = [];
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;

function connectWebSocket(): WebSocket {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  const tokenParam = authToken ? `?token=${encodeURIComponent(authToken)}` : "";
  const wsUrl = `${scheme}://${window.location.host}/ws${tokenParam}`;
  const ws = new WebSocket(wsUrl);

  ws.addEventListener("open", () => {
    reconnectDelay = 1000; // reset on success
  });

  ws.addEventListener("message", (msg) => {
    for (const listener of wsListeners) {
      try {
        listener(msg);
      } catch (e) {
        console.error("WebSocket listener error:", e);
      }
    }
  });

  ws.addEventListener("close", (ev) => {
    // Code 1008 = Policy Violation — server rejected the handshake (bad/missing token)
    if (ev.code === 1008) {
      authError.set(true);
    }
    // Only reconnect if this is still the active WebSocket
    if (sharedWs !== ws) return;
    if (reconnectTimer) return;
    // Don't reconnect if auth has failed — avoids infinite retry spam
    if (get(authError)) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      if (sharedWs === ws) {
        sharedWs = null;
        sharedWs = connectWebSocket();
      }
    }, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, 30000);
  });

  return ws;
}

function getSharedWebSocket(): WebSocket {
  if (!sharedWs || sharedWs.readyState === WebSocket.CLOSED || sharedWs.readyState === WebSocket.CLOSING) {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    sharedWs = connectWebSocket();
  }
  return sharedWs;
}

export async function command<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (authToken) headers["Authorization"] = `Bearer ${authToken}`;
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers,
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) {
    if (res.status === 401) {
      authError.set(true);
      throw new Error("Unauthorized");
    }
    throw new Error(await res.text());
  }
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

  getSharedWebSocket(); // ensure connected
  const callback = (msg: MessageEvent) => {
    const data = JSON.parse(msg.data);
    if (data.event === event) handler(data.payload);
  };
  wsListeners.push(callback);
  return () => {
    wsListeners = wsListeners.filter((l) => l !== callback);
  };
}
