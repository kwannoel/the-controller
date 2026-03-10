import { mkdirSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";

const CONTROLLER_DIR = path.join(homedir(), ".the-controller");
const PROJECTS_DIR = path.join(CONTROLLER_DIR, "projects");

export interface SeededProject {
  projectId: string;
  sessionId: string;
  projectDir: string;
}

/**
 * Write a project.json into ~/.the-controller/projects/<uuid>/ so the app
 * picks it up on launch. The project points at `repoPath` and has one session
 * on `branchName`.
 */
export function seedProject(
  repoPath: string,
  branchName: string
): SeededProject {
  const projectId = randomUUID();
  const sessionId = randomUUID();
  const projectDir = path.join(PROJECTS_DIR, projectId);

  mkdirSync(projectDir, { recursive: true });

  const project = {
    id: projectId,
    name: `e2e-test-${Date.now()}`,
    repo_path: repoPath,
    created_at: new Date().toISOString(),
    archived: false,
    maintainer: { enabled: false, interval_minutes: 30 },
    auto_worker: { enabled: false },
    prompts: [],
    staged_session: null,
    sessions: [
      {
        id: sessionId,
        label: branchName,
        worktree_path: repoPath, // In our test, the clone IS the worktree
        worktree_branch: branchName,
        archived: false,
        kind: "codex",
        github_issue: null,
        initial_prompt: null,
        done_commits: [],
        auto_worker_session: false,
      },
    ],
  };

  writeFileSync(
    path.join(projectDir, "project.json"),
    JSON.stringify(project, null, 2)
  );

  return { projectId, sessionId, projectDir };
}

/**
 * Remove the seeded project directory.
 */
export function cleanupSeededProject(seeded: SeededProject): void {
  if (existsSync(seeded.projectDir)) {
    rmSync(seeded.projectDir, { recursive: true, force: true });
  }
}
