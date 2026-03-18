import App from "./App.svelte";
import "./app.css";
import { mount } from "svelte";

const isTauri = !!(window as any).__TAURI_INTERNALS__;

function logToBackend(message: string) {
  if (isTauri) {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("log_frontend_error", { message }).catch(() => {});
    }).catch(() => {});
  } else {
    console.error("[frontend]", message);
  }
}

window.addEventListener("error", (e) => {
  const loc = e.filename ? ` at ${e.filename}:${e.lineno}:${e.colno}` : "";
  logToBackend(`${e.message}${loc}\n${e.error?.stack || ""}`);
});

window.addEventListener("unhandledrejection", (e) => {
  const reason = e.reason instanceof Error
    ? `${e.reason.message}\n${e.reason.stack}`
    : String(e.reason);
  logToBackend(`Unhandled rejection: ${reason}`);
});

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
