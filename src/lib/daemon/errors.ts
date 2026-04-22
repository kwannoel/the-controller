import { DaemonHttpError } from "./client";

export type ErrorKind =
  | "auth"          // 401
  | "not_found"     // 404
  | "session_ended" // 409
  | "invalid"       // 422
  | "storage"       // 503
  | "network"       // fetch failed
  | "unknown";      // anything else

export interface ClassifiedError {
  kind: ErrorKind;
  message: string;
  status: number | null;
}

export function classifyError(err: unknown): ClassifiedError {
  if (
    err instanceof DaemonHttpError ||
    (err != null && typeof err === "object" && (err as { name?: unknown }).name === "DaemonHttpError")
  ) {
    const e = err as DaemonHttpError;
    const kind: ErrorKind =
      e.status === 401 ? "auth" :
      e.status === 404 ? "not_found" :
      e.status === 409 ? "session_ended" :
      e.status === 422 ? "invalid" :
      e.status === 503 ? "storage" :
      "unknown";
    return { kind, message: e.message, status: e.status };
  }
  if (err instanceof TypeError) {
    return { kind: "network", message: err.message, status: null };
  }
  if (err instanceof Error) {
    return { kind: "unknown", message: err.message, status: null };
  }
  return { kind: "unknown", message: String(err), status: null };
}
