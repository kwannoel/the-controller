import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { globSync } from "glob";

function read(path: string): string {
  return readFileSync(path, "utf8");
}

function serverRoutes(): Set<string> {
  const source = read("server/src/main.rs");
  return new Set([...source.matchAll(/"\/api\/([a-zA-Z0-9_]+)"/g)].map((m) => m[1]));
}

function productionFrontendCommands(): Set<string> {
  const files = globSync("src/**/*.{ts,svelte}", {
    ignore: ["src/**/*.test.ts", "src/**/*.test.svelte.ts", "src/**/__mocks__/**"],
  });
  const commands = new Set<string>();

  for (const file of files) {
    const source = read(file);
    for (const match of source.matchAll(/command(?:<[^>]+>)?\(\s*["']([a-zA-Z0-9_]+)["']/g)) {
      commands.add(match[1]);
    }
  }

  return commands;
}

describe("web backend migration audit", () => {
  it("keeps every production frontend command backed by an HTTP route", () => {
    const routes = serverRoutes();
    const missing = [...productionFrontendCommands()].filter((cmd) => !routes.has(cmd));

    expect(missing).toEqual([]);
  });

  it("keeps the desktop command surface covered by routes or browser replacements", () => {
    const routes = serverRoutes();
    const requiredRoutes = [
      "restore_sessions",
      "connect_session",
      "create_project",
      "load_project",
      "list_projects",
      "delete_project",
      "get_agents_md",
      "update_agents_md",
      "create_session",
      "write_to_pty",
      "send_raw_to_pty",
      "resize_pty",
      "close_session",
      "set_initial_prompt",
      "submit_secure_env_value",
      "cancel_secure_env_request",
      "start_claude_login",
      "stop_claude_login",
      "home_dir",
      "check_onboarding",
      "save_onboarding_config",
      "check_claude_cli",
      "list_directories_at",
      "list_root_directories",
      "generate_project_names",
      "scaffold_project",
      "list_github_issues",
      "kanban_load_order",
      "kanban_save_order",
      "list_assigned_issues",
      "generate_issue_body",
      "create_github_issue",
      "close_github_issue",
      "delete_github_issue",
      "post_github_comment",
      "add_github_label",
      "remove_github_label",
      "merge_session_branch",
      "get_session_commits",
      "configure_maintainer",
      "get_maintainer_status",
      "get_maintainer_history",
      "trigger_maintainer_check",
      "clear_maintainer_reports",
      "get_maintainer_issues",
      "get_maintainer_issue_detail",
      "configure_auto_worker",
      "get_auto_worker_queue",
      "get_worker_reports",
      "save_session_prompt",
      "list_project_prompts",
      "stage_session",
      "unstage_session",
      "get_repo_head",
      "get_session_token_usage",
      "log_frontend_error",
      "read_daemon_token",
      "save_screenshot",
    ];

    const missing = requiredRoutes.filter((route) => !routes.has(route));
    expect(missing).toEqual([]);

    const nativeReplacements = read("src/lib/native.ts");
    expect(nativeReplacements).toContain("html2canvas");
    expect(nativeReplacements).toContain("ClipboardItem");
  });

  it("keeps active docs aligned with the web frontend plus backend runtime", () => {
    const activeDocs = [
      "README.md",
      "ARCHITECTURE.md",
      "docs/keyboard-modes.md",
      "e2e/specs/chat-mode.spec.ts",
    ];

    const stalePatterns = [
      /Built with Tauri/i,
      /Tauri v2/i,
      /npm run tauri dev/i,
      /src-tauri\/src/i,
      /six workspace modes/i,
      /Ambient Mode . Architecture Keys/i,
      /Ambient Mode . Notes Keys/i,
      /Ambient Mode . Infrastructure Keys/i,
      /Ambient Mode . Voice Keys/i,
      /read_daemon_token`[\s\S]{0,120}not exposed/i,
    ];

    const stale = activeDocs.flatMap((file) => {
      const source = read(file);
      return stalePatterns
        .filter((pattern) => pattern.test(source))
        .map((pattern) => `${file}: ${pattern}`);
    });

    expect(stale).toEqual([]);
  });
});
