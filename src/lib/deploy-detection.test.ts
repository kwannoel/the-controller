import { describe, it, expect } from "vitest";
import { classifyProject, type ProjectSignals } from "./deploy-detection";

const defaults: ProjectSignals = {
  has_dockerfile: false,
  has_package_json: false,
  has_vite_config: false,
  has_start_script: false,
  has_pyproject: false,
};

describe("classifyProject", () => {
  it("detects static site (vite config, no start script)", () => {
    expect(classifyProject({ ...defaults, has_vite_config: true, has_package_json: true })).toBe("static");
  });

  it("detects docker when Dockerfile exists", () => {
    expect(classifyProject({ ...defaults, has_dockerfile: true })).toBe("docker");
  });

  it("detects node service (package.json with start script)", () => {
    expect(classifyProject({ ...defaults, has_package_json: true, has_start_script: true })).toBe("node");
  });

  it("detects python (pyproject.toml)", () => {
    expect(classifyProject({ ...defaults, has_pyproject: true })).toBe("python");
  });

  it("returns unknown when nothing matches", () => {
    expect(classifyProject(defaults)).toBe("unknown");
  });

  it("prefers static over node when both vite config and package.json exist", () => {
    expect(classifyProject({ ...defaults, has_vite_config: true, has_package_json: true })).toBe("static");
  });

  it("prefers docker over node when both Dockerfile and package.json exist", () => {
    expect(classifyProject({ ...defaults, has_dockerfile: true, has_package_json: true, has_start_script: true })).toBe("docker");
  });
});
