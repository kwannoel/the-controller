import { describe, it, expect } from "vitest";
import { classifyError } from "./errors";
import { DaemonHttpError } from "./client";

describe("classifyError", () => {
  it("classifies 401 as auth", () => {
    const c = classifyError(new DaemonHttpError(401, "", "unauthorized"));
    expect(c.kind).toBe("auth");
  });
  it("classifies 404 as not_found", () => {
    const c = classifyError(new DaemonHttpError(404, "", "not found"));
    expect(c.kind).toBe("not_found");
  });
  it("classifies 409 as session_ended", () => {
    const c = classifyError(new DaemonHttpError(409, "", "ended"));
    expect(c.kind).toBe("session_ended");
  });
  it("classifies 422 as invalid", () => {
    const c = classifyError(new DaemonHttpError(422, "", "invalid"));
    expect(c.kind).toBe("invalid");
  });
  it("classifies 503 as storage", () => {
    const c = classifyError(new DaemonHttpError(503, "", "db down"));
    expect(c.kind).toBe("storage");
  });
  it("classifies network TypeError as network", () => {
    const c = classifyError(new TypeError("Failed to fetch"));
    expect(c.kind).toBe("network");
  });
  it("classifies unknown Error as unknown", () => {
    const c = classifyError(new Error("boom"));
    expect(c.kind).toBe("unknown");
  });
  it("classifies unknown status (500) as unknown", () => {
    const c = classifyError(new DaemonHttpError(500, "", "server"));
    expect(c.kind).toBe("unknown");
  });
});
