import { describe, it, expect } from "vitest";
import { groupSessionsByProject } from "./grouping";

describe("groupSessionsByProject", () => {
  it("buckets sessions by project.repo_path; unmatched into Other", () => {
    const projects = [
      { id: "p1", name: "A", repo_path: "/tmp/a" },
      { id: "p2", name: "B", repo_path: "/tmp/b" },
    ] as any;
    const sessions = [
      { id: "s1", cwd: "/tmp/a" },
      { id: "s2", cwd: "/tmp/a" },
      { id: "s3", cwd: "/other" },
    ] as any;
    const groups = groupSessionsByProject(projects, sessions);
    expect(groups.byProject.get("p1")!.map((s: any) => s.id)).toEqual(["s1", "s2"]);
    expect(groups.byProject.get("p2")).toEqual([]);
    expect(groups.other.map((s: any) => s.id)).toEqual(["s3"]);
  });

  it("returns empty byProject entries for projects with no sessions", () => {
    const projects = [{ id: "p1", name: "A", repo_path: "/tmp/a" }] as any;
    const groups = groupSessionsByProject(projects, []);
    expect(groups.byProject.get("p1")).toEqual([]);
    expect(groups.other).toEqual([]);
  });
});
