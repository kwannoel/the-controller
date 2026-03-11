# Deployment Workspace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add an Infrastructure workspace to The Controller that deploys projects to Hetzner + Cloudflare via Coolify, with hotkey-driven deploys and a monitoring dashboard.

**Architecture:** New "infrastructure" workspace mode with Svelte frontend + Rust backend. Backend integrates with three external APIs (Hetzner, Cloudflare, Coolify) via HTTP clients. Frontend follows existing workspace patterns (AgentDashboard, NotesEditor). Deploy flow: hotkey → project type detection → Coolify API → Cloudflare DNS → health check → notification.

**Tech Stack:** Svelte 5 (runes), Rust (Tauri v2), reqwest (HTTP client), Coolify API v1, Hetzner Cloud API, Cloudflare API v4

**Design doc:** `docs/plans/2026-03-11-deployment-workspace-design.md`

---

## Phase 1: Workspace Shell

Register the "infrastructure" workspace mode and render an empty component. This follows the exact pattern used for Notes, Agents, and Architecture workspaces.

### Task 1: Add `infrastructure` workspace mode type

**Files:**
- Modify: `src/lib/stores.ts:176-181`
- Modify: `src/lib/focus-helpers.ts:53-99`
- Test: `src/lib/focus-helpers.test.ts` (create if needed, otherwise add tests)

**Step 1: Write the failing test**

Check if a test file exists for focus-helpers. If not, create `src/lib/focus-helpers.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { focusForModeSwitch } from "./focus-helpers";
import type { FocusTarget, Project } from "./stores";

function makeProject(id: string): Project {
  return {
    id,
    name: `project-${id}`,
    repo_path: `/tmp/${id}`,
    created_at: new Date().toISOString(),
    archived: false,
    sessions: [],
    maintainer: { enabled: false, interval_minutes: 30 },
    auto_worker: { enabled: false },
    prompts: [],
    staged_session: null,
  };
}

describe("focusForModeSwitch", () => {
  it("switches session focus to project when entering infrastructure mode", () => {
    const focus: FocusTarget = { type: "session", sessionId: "s1", projectId: "p1" };
    const result = focusForModeSwitch(focus, "infrastructure", "s1", [makeProject("p1")]);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("switches agent focus to project when entering infrastructure mode", () => {
    const focus: FocusTarget = { type: "agent", agentKind: "maintainer", projectId: "p1" };
    const result = focusForModeSwitch(focus, "infrastructure", null, [makeProject("p1")]);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("keeps project focus when entering infrastructure mode", () => {
    const focus: FocusTarget = { type: "project", projectId: "p1" };
    const result = focusForModeSwitch(focus, "infrastructure", null, [makeProject("p1")]);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: FAIL — `"infrastructure"` is not assignable to type `WorkspaceMode`

**Step 3: Update stores.ts — add `infrastructure` to WorkspaceMode**

In `src/lib/stores.ts:176-181`, add `"infrastructure"` to the union:

```typescript
export type WorkspaceMode =
  | "development"
  | "agents"
  | "notes"
  | "architecture"
  | "infrastructure";
```

**Step 4: Update focus-helpers.ts — add infrastructure mode switch**

In `src/lib/focus-helpers.ts`, add after the `architecture` block (before `return current`):

```typescript
  if (newMode === "infrastructure") {
    if (
      current.type === "session" ||
      current.type === "agent" ||
      current.type === "agent-panel" ||
      current.type === "note" ||
      current.type === "notes-editor"
    ) {
      return { type: "project", projectId: current.projectId };
    }
  }
```

Also update the `development` branch to include infrastructure focus types — when switching FROM infrastructure TO development, translate project focus to active session. No change needed since `"project"` type already passes through.

**Step 5: Run test to verify it passes**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/stores.ts src/lib/focus-helpers.ts src/lib/focus-helpers.test.ts
git commit -m "feat: add infrastructure workspace mode type"
```

---

### Task 2: Add infrastructure workspace to mode picker and hotkey handler

**Files:**
- Modify: `src/lib/WorkspaceModePicker.svelte:8-13`
- Modify: `src/lib/HotkeyManager.svelte:106-134`
- Test: `src/lib/HotkeyManager.test.ts` (add test)

**Step 1: Write the failing test**

In `src/lib/HotkeyManager.test.ts`, add a test for workspace switching to infrastructure. Follow existing workspace mode test patterns in the file:

```typescript
it("Space → i switches to infrastructure workspace", async () => {
  // Setup: render HotkeyManager, simulate non-terminal focus
  pressKey(" ");  // Space opens workspace picker
  pressKey("i");  // i selects infrastructure
  // Assert workspaceMode store value
  expect(get(workspaceMode)).toBe("infrastructure");
});
```

Adapt this to match the existing test patterns in the file (check how other `Space → <key>` tests work).

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: FAIL — `workspaceMode` is still the previous value, since `"i"` isn't handled yet

**Step 3: Update WorkspaceModePicker.svelte**

In `src/lib/WorkspaceModePicker.svelte:8-13`, add infrastructure to the modes array:

```typescript
  const modes: { key: string; id: WorkspaceMode; label: string }[] = [
    { key: "d", id: "development", label: "Development" },
    { key: "a", id: "agents", label: "Agents" },
    { key: "r", id: "architecture", label: "Architecture" },
    { key: "n", id: "notes", label: "Notes" },
    { key: "i", id: "infrastructure", label: "Infrastructure" },
  ];
```

**Step 4: Update HotkeyManager.svelte**

In `src/lib/HotkeyManager.svelte`, in the `handleWorkspaceModeKey` function (around line 106-134), add after the `"n"` (notes) block:

```typescript
    if (key === "i") {
      workspaceMode.set("infrastructure");
      const newFocus = focusForModeSwitch(currentFocus, "infrastructure", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
```

**Step 5: Run test to verify it passes**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/WorkspaceModePicker.svelte src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "feat: add infrastructure mode to workspace picker and hotkey handler"
```

---

### Task 3: Create empty InfrastructureDashboard component

**Files:**
- Create: `src/lib/InfrastructureDashboard.svelte`
- Modify: `src/App.svelte:473-496`
- Test: `src/lib/InfrastructureDashboard.test.ts`

**Step 1: Write the failing test**

Create `src/lib/InfrastructureDashboard.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import InfrastructureDashboard from "./InfrastructureDashboard.svelte";

describe("InfrastructureDashboard", () => {
  it("renders the empty state", () => {
    render(InfrastructureDashboard);
    expect(screen.getByText("Infrastructure")).toBeTruthy();
    expect(screen.getByText(/no services deployed/i)).toBeTruthy();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/InfrastructureDashboard.test.ts`
Expected: FAIL — module not found

**Step 3: Create InfrastructureDashboard.svelte**

Create `src/lib/InfrastructureDashboard.svelte`:

```svelte
<script lang="ts">
</script>

<div class="container">
  <div class="empty-state">
    <div class="title">Infrastructure</div>
    <div class="subtitle">No services deployed yet</div>
    <div class="hint">Deploy a project with <kbd>Leader</kbd> → <kbd>d</kbd> from a session</div>
  </div>
</div>

<style>
  .container {
    height: 100%;
    background: #11111b;
    color: #cdd6f4;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-state {
    text-align: center;
  }

  .title {
    font-size: 18px;
    font-weight: 600;
    margin-bottom: 8px;
  }

  .subtitle {
    font-size: 14px;
    color: #a6adc8;
    margin-bottom: 16px;
  }

  .hint {
    font-size: 12px;
    color: #6c7086;
  }

  kbd {
    background: #313244;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 11px;
  }
</style>
```

**Step 4: Wire into App.svelte**

In `src/App.svelte`, add the import at the top (after ArchitectureExplorer import):

```typescript
import InfrastructureDashboard from "./lib/InfrastructureDashboard.svelte";
```

In the workspace rendering block (`src/App.svelte:473-496`), add before the `{:else}` (TerminalManager) branch:

```svelte
        {:else if workspaceModeState.current === "infrastructure"}
          <InfrastructureDashboard />
```

**Step 5: Run test to verify it passes**

Run: `npx vitest run src/lib/InfrastructureDashboard.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/InfrastructureDashboard.svelte src/lib/InfrastructureDashboard.test.ts src/App.svelte
git commit -m "feat: add empty InfrastructureDashboard workspace component"
```

---

### Task 4: Add infrastructure command section and hotkeys

**Files:**
- Modify: `src/lib/commands.ts`

**Step 1: Write the failing test**

In a test file (create `src/lib/commands.test.ts` if needed):

```typescript
import { describe, it, expect } from "vitest";
import { buildKeyMap, getHelpSections } from "./commands";

describe("infrastructure commands", () => {
  it("includes deploy-project command in infrastructure mode keymap", () => {
    const map = buildKeyMap("infrastructure");
    expect(map.get("d")).toBe("deploy-project");
  });

  it("includes Infrastructure section in help for infrastructure mode", () => {
    const sections = getHelpSections("infrastructure");
    const infraSection = sections.find(s => s.label === "Infrastructure");
    expect(infraSection).toBeTruthy();
    expect(infraSection!.entries.length).toBeGreaterThan(0);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: FAIL — no `deploy-project` command exists

**Step 3: Add infrastructure commands to commands.ts**

In `src/lib/commands.ts`, add `"deploy-project"` and `"rollback-deploy"` to the `CommandId` type:

```typescript
export type CommandId =
  | "navigate-next"
  // ... existing IDs ...
  | "toggle-maintainer-view"
  | "deploy-project"
  | "rollback-deploy";
```

Add `"Infrastructure"` to the `CommandSection` type:

```typescript
export type CommandSection = "Navigation" | "Sessions" | "Projects" | "Panels" | "Agents" | "Notes" | "Infrastructure";
```

Add `"Infrastructure"` to the `SECTION_ORDER` array:

```typescript
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels", "Agents", "Notes", "Infrastructure"];
```

Add infrastructure commands to the `commands` array:

```typescript
  // ── Infrastructure ──
  { id: "deploy-project", key: "d", section: "Infrastructure", description: "Deploy focused project", mode: "infrastructure" },
  { id: "rollback-deploy", key: "r", section: "Infrastructure", description: "Rollback last deployment", mode: "infrastructure" },
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: PASS

**Step 5: Update HotkeyManager.svelte handleHotkey switch**

In `src/lib/HotkeyManager.svelte`, in the `handleHotkey` function's switch statement, add cases before the `default`:

```typescript
      case "deploy-project": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({ type: "deploy-project", projectId: project.id, repoPath: project.repo_path });
        }
        return true;
      }
      case "rollback-deploy": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({ type: "rollback-deploy", projectId: project.id });
        }
        return true;
      }
```

**Step 6: Add HotkeyAction types in stores.ts**

In `src/lib/stores.ts`, add to the `HotkeyAction` union (before `| null`):

```typescript
  | { type: "deploy-project"; projectId: string; repoPath: string }
  | { type: "rollback-deploy"; projectId: string }
```

**Step 7: Commit**

```bash
git add src/lib/commands.ts src/lib/commands.test.ts src/lib/HotkeyManager.svelte src/lib/stores.ts
git commit -m "feat: add infrastructure hotkey commands (deploy, rollback)"
```

---

## Phase 2: Credential Storage & Onboarding

Store Hetzner, Cloudflare, and Coolify API credentials encrypted. Add an onboarding modal for first-time setup.

### Task 5: Add deploy credential types and storage (Rust)

**Files:**
- Create: `src-tauri/src/deploy/mod.rs`
- Create: `src-tauri/src/deploy/credentials.rs`
- Modify: `src-tauri/src/lib.rs` (add module)

**Step 1: Write the failing test**

Create `src-tauri/src/deploy/credentials.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployCredentials {
    pub hetzner_api_key: Option<String>,
    pub cloudflare_api_key: Option<String>,
    pub cloudflare_zone_id: Option<String>,
    pub root_domain: Option<String>,
    pub coolify_url: Option<String>,
    pub coolify_api_key: Option<String>,
    pub server_ip: Option<String>,
}

impl DeployCredentials {
    pub fn is_provisioned(&self) -> bool {
        self.hetzner_api_key.is_some()
            && self.cloudflare_api_key.is_some()
            && self.root_domain.is_some()
            && self.coolify_url.is_some()
            && self.coolify_api_key.is_some()
            && self.server_ip.is_some()
    }

    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".the-controller")
            .join("deploy-credentials.json")
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_credentials_are_not_provisioned() {
        let creds = DeployCredentials::default();
        assert!(!creds.is_provisioned());
    }

    #[test]
    fn test_fully_populated_credentials_are_provisioned() {
        let creds = DeployCredentials {
            hetzner_api_key: Some("hk".into()),
            cloudflare_api_key: Some("cf".into()),
            cloudflare_zone_id: Some("zone".into()),
            root_domain: Some("example.com".into()),
            coolify_url: Some("https://coolify.example.com".into()),
            coolify_api_key: Some("ck".into()),
            server_ip: Some("1.2.3.4".into()),
        };
        assert!(creds.is_provisioned());
    }

    #[test]
    fn test_partial_credentials_are_not_provisioned() {
        let creds = DeployCredentials {
            hetzner_api_key: Some("hk".into()),
            ..Default::default()
        };
        assert!(!creds.is_provisioned());
    }

    #[test]
    fn test_credentials_serialize_roundtrip() {
        let creds = DeployCredentials {
            hetzner_api_key: Some("test-key".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&creds).unwrap();
        let deserialized: DeployCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hetzner_api_key, Some("test-key".into()));
    }
}
```

**Step 2: Create deploy module**

Create `src-tauri/src/deploy/mod.rs`:

```rust
pub mod credentials;
```

**Step 3: Register module in lib.rs**

In `src-tauri/src/lib.rs`, add `pub mod deploy;` after the other module declarations.

**Step 4: Run tests**

Run: `cd src-tauri && cargo test deploy::credentials`
Expected: PASS (all 4 tests)

**Step 5: Commit**

```bash
git add src-tauri/src/deploy/
git add src-tauri/src/lib.rs
git commit -m "feat: add deploy credential storage module"
```

---

### Task 6: Add Tauri commands for credential management

**Files:**
- Create: `src-tauri/src/deploy/commands.rs`
- Modify: `src-tauri/src/deploy/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register commands)

**Step 1: Create deploy commands**

Create `src-tauri/src/deploy/commands.rs`:

```rust
use super::credentials::DeployCredentials;

#[tauri::command]
pub async fn get_deploy_credentials() -> Result<DeployCredentials, String> {
    tokio::task::spawn_blocking(|| DeployCredentials::load())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_deploy_credentials(credentials: DeployCredentials) -> Result<(), String> {
    tokio::task::spawn_blocking(move || credentials.save())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn is_deploy_provisioned() -> Result<bool, String> {
    tokio::task::spawn_blocking(|| {
        let creds = DeployCredentials::load()?;
        Ok(creds.is_provisioned())
    })
    .await
    .map_err(|e| e.to_string())?
}
```

**Step 2: Update deploy/mod.rs**

```rust
pub mod commands;
pub mod credentials;
```

**Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the `generate_handler![]` macro:

```rust
            deploy::commands::get_deploy_credentials,
            deploy::commands::save_deploy_credentials,
            deploy::commands::is_deploy_provisioned,
```

**Step 4: Run Rust tests to ensure compilation**

Run: `cd src-tauri && cargo test`
Expected: PASS (compiles and existing tests pass)

**Step 5: Commit**

```bash
git add src-tauri/src/deploy/commands.rs src-tauri/src/deploy/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add Tauri commands for deploy credential management"
```

---

### Task 7: Add deploy onboarding modal (frontend)

**Files:**
- Create: `src/lib/DeploySetupModal.svelte`
- Create: `src/lib/DeploySetupModal.test.ts`
- Modify: `src/App.svelte`

**Step 1: Write the failing test**

Create `src/lib/DeploySetupModal.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import DeploySetupModal from "./DeploySetupModal.svelte";

vi.mock("$lib/backend", () => ({
  command: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockReturnValue(vi.fn()),
}));

describe("DeploySetupModal", () => {
  it("renders step 1 with Hetzner API key input", () => {
    render(DeploySetupModal, { onComplete: vi.fn(), onClose: vi.fn() });
    expect(screen.getByText(/hetzner/i)).toBeTruthy();
  });

  it("calls onClose when close button is clicked", async () => {
    const onClose = vi.fn();
    render(DeploySetupModal, { onComplete: vi.fn(), onClose });
    const closeBtn = screen.getByRole("button", { name: /cancel/i });
    await fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/DeploySetupModal.test.ts`
Expected: FAIL — module not found

**Step 3: Create DeploySetupModal.svelte**

Create `src/lib/DeploySetupModal.svelte` following the SecureEnvModal pattern:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";

  interface Props {
    onComplete: () => void;
    onClose: () => void;
  }

  let { onComplete, onClose }: Props = $props();

  let step = $state(1);
  let hetznerKey = $state("");
  let cloudflareKey = $state("");
  let rootDomain = $state("");
  let provisioning = $state(false);
  let error = $state<string | null>(null);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(() => inputEl?.focus());

  async function handleNext() {
    if (step === 1 && hetznerKey.trim()) {
      step = 2;
      setTimeout(() => inputEl?.focus(), 50);
    } else if (step === 2 && cloudflareKey.trim() && rootDomain.trim()) {
      step = 3;
      await provision();
    }
  }

  async function provision() {
    provisioning = true;
    error = null;
    try {
      await command("save_deploy_credentials", {
        credentials: {
          hetzner_api_key: hetznerKey.trim(),
          cloudflare_api_key: cloudflareKey.trim(),
          cloudflare_zone_id: null,
          root_domain: rootDomain.trim(),
          coolify_url: null,
          coolify_api_key: null,
          server_ip: null,
        },
      });
      // TODO: Phase 3 will add server provisioning here
      onComplete();
    } catch (e) {
      error = String(e);
      provisioning = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleNext();
    if (e.key === "Escape") onClose();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="overlay" onclick={onClose} role="dialog" onkeydown={handleKeydown}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <div class="modal-title">Deploy Setup — Step {step} of 3</div>

    {#if step === 1}
      <label class="field-label">Hetzner API Key</label>
      <input
        bind:this={inputEl}
        bind:value={hetznerKey}
        type="password"
        class="field-input"
        placeholder="Enter your Hetzner Cloud API token"
      />
      <p class="hint">Get one from Hetzner Cloud Console → Security → API Tokens</p>
    {:else if step === 2}
      <label class="field-label">Cloudflare API Key</label>
      <input
        bind:this={inputEl}
        bind:value={cloudflareKey}
        type="password"
        class="field-input"
        placeholder="Enter your Cloudflare API token"
      />
      <label class="field-label">Root Domain</label>
      <input
        bind:value={rootDomain}
        type="text"
        class="field-input"
        placeholder="e.g. yourdomain.com"
      />
    {:else if step === 3}
      {#if provisioning}
        <p class="status">Provisioning server...</p>
      {:else if error}
        <p class="error">{error}</p>
      {/if}
    {/if}

    <div class="actions">
      <button class="btn cancel" onclick={onClose}>Cancel</button>
      {#if step < 3}
        <button class="btn primary" onclick={handleNext}>Next</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 120;
  }

  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    padding: 24px;
    min-width: 400px;
    max-width: 480px;
  }

  .modal-title {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
    margin-bottom: 20px;
  }

  .field-label {
    display: block;
    font-size: 12px;
    color: #a6adc8;
    margin-bottom: 6px;
    margin-top: 12px;
  }

  .field-input {
    width: 100%;
    padding: 8px 12px;
    background: #11111b;
    border: 1px solid #313244;
    border-radius: 4px;
    color: #cdd6f4;
    font-size: 13px;
    outline: none;
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: #89b4fa;
  }

  .hint {
    font-size: 11px;
    color: #6c7086;
    margin-top: 6px;
  }

  .status {
    color: #89b4fa;
    font-size: 14px;
  }

  .error {
    color: #f38ba8;
    font-size: 13px;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }

  .btn {
    padding: 6px 16px;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
    border: none;
  }

  .cancel {
    background: #313244;
    color: #a6adc8;
  }

  .primary {
    background: #89b4fa;
    color: #1e1e2e;
    font-weight: 600;
  }
</style>
```

**Step 4: Wire into App.svelte**

Import the modal:
```typescript
import DeploySetupModal from "./lib/DeploySetupModal.svelte";
```

Add state:
```typescript
let deploySetupOpen = $state(false);
```

Handle the hotkey action in the `$effect` block — add a case:
```typescript
      } else if (action?.type === "deploy-project") {
        // Check if provisioned, if not open setup
        command<boolean>("is_deploy_provisioned").then((provisioned) => {
          if (!provisioned) {
            deploySetupOpen = true;
          } else {
            // TODO: Phase 4 will trigger actual deploy
            showToast("Deploy not yet implemented", "info");
          }
        });
```

Add modal render before the closing `{/if}` (after WorkspaceModePicker):
```svelte
    {#if deploySetupOpen}
      <DeploySetupModal
        onComplete={() => { deploySetupOpen = false; showToast("Deploy setup complete", "info"); }}
        onClose={() => { deploySetupOpen = false; }}
      />
    {/if}
```

**Step 5: Run tests**

Run: `npx vitest run src/lib/DeploySetupModal.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/DeploySetupModal.svelte src/lib/DeploySetupModal.test.ts src/App.svelte
git commit -m "feat: add deploy setup onboarding modal"
```

---

## Phase 3: Coolify API Client (Rust)

HTTP client for Coolify's API to manage applications, deployments, and get logs/metrics.

### Task 8: Add reqwest dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add reqwest to dependencies**

In `src-tauri/Cargo.toml`, add:

```toml
reqwest = { version = "0.12", features = ["json"] }
```

**Step 2: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add reqwest HTTP client dependency"
```

---

### Task 9: Coolify API client — application management

**Files:**
- Create: `src-tauri/src/deploy/coolify.rs`
- Modify: `src-tauri/src/deploy/mod.rs`

**Step 1: Write the failing test**

Create `src-tauri/src/deploy/coolify.rs` with types and the client:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolifyApp {
    pub uuid: String,
    pub name: String,
    pub fqdn: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolifyDeployment {
    pub id: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CoolifyClient {
    base_url: String,
    api_key: String,
    client: Client,
}

impl CoolifyClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub async fn list_applications(&self) -> Result<Vec<CoolifyApp>, String> {
        let resp = self.client
            .get(format!("{}/api/v1/applications", self.base_url))
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Coolify API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Coolify API error {status}: {body}"));
        }

        resp.json::<Vec<CoolifyApp>>()
            .await
            .map_err(|e| format!("Failed to parse Coolify response: {e}"))
    }

    pub async fn deploy_application(&self, uuid: &str) -> Result<(), String> {
        let resp = self.client
            .post(format!("{}/api/v1/applications/{uuid}/restart", self.base_url))
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Coolify deploy request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Coolify deploy error {status}: {body}"));
        }

        Ok(())
    }

    pub async fn get_deployments(&self, uuid: &str) -> Result<Vec<CoolifyDeployment>, String> {
        let resp = self.client
            .get(format!("{}/api/v1/applications/{uuid}/deployments", self.base_url))
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Coolify API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Coolify API error {status}: {body}"));
        }

        resp.json::<Vec<CoolifyDeployment>>()
            .await
            .map_err(|e| format!("Failed to parse Coolify deployments: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_construction() {
        let client = CoolifyClient::new("https://coolify.example.com/", "test-key");
        assert_eq!(client.base_url, "https://coolify.example.com");
        assert_eq!(client.api_key, "test-key");
    }

    #[test]
    fn test_auth_header_format() {
        let client = CoolifyClient::new("https://coolify.example.com", "my-token");
        assert_eq!(client.auth_header(), "Bearer my-token");
    }

    #[test]
    fn test_coolify_app_deserialize() {
        let json = r#"{"uuid":"abc-123","name":"myapp","fqdn":"https://myapp.example.com","status":"running","description":null}"#;
        let app: CoolifyApp = serde_json::from_str(json).unwrap();
        assert_eq!(app.uuid, "abc-123");
        assert_eq!(app.name, "myapp");
        assert_eq!(app.status, Some("running".to_string()));
    }
}
```

**Step 2: Update deploy/mod.rs**

```rust
pub mod commands;
pub mod coolify;
pub mod credentials;
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test deploy::coolify`
Expected: PASS (3 tests)

**Step 4: Commit**

```bash
git add src-tauri/src/deploy/coolify.rs src-tauri/src/deploy/mod.rs
git commit -m "feat: add Coolify API client with app and deployment management"
```

---

### Task 10: Cloudflare DNS client

**Files:**
- Create: `src-tauri/src/deploy/cloudflare.rs`
- Modify: `src-tauri/src/deploy/mod.rs`

**Step 1: Create cloudflare.rs**

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
    pub proxied: bool,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    result: Option<T>,
    errors: Vec<CloudflareError>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    message: String,
}

#[derive(Debug, Clone)]
pub struct CloudflareClient {
    api_key: String,
    zone_id: String,
    client: Client,
}

impl CloudflareClient {
    pub fn new(api_key: &str, zone_id: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            zone_id: zone_id.to_string(),
            client: Client::new(),
        }
    }

    pub async fn create_dns_record(
        &self,
        subdomain: &str,
        server_ip: &str,
    ) -> Result<DnsRecord, String> {
        let body = serde_json::json!({
            "type": "A",
            "name": subdomain,
            "content": server_ip,
            "proxied": true,
            "ttl": 1,
        });

        let resp = self.client
            .post(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                self.zone_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Cloudflare API request failed: {e}"))?;

        let cf_resp: CloudflareResponse<DnsRecord> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Cloudflare response: {e}"))?;

        if !cf_resp.success {
            let msgs: Vec<String> = cf_resp.errors.iter().map(|e| e.message.clone()).collect();
            return Err(format!("Cloudflare error: {}", msgs.join(", ")));
        }

        cf_resp.result.ok_or_else(|| "No result in Cloudflare response".to_string())
    }

    pub async fn list_dns_records(&self) -> Result<Vec<DnsRecord>, String> {
        let resp = self.client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                self.zone_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("Cloudflare API request failed: {e}"))?;

        let cf_resp: CloudflareResponse<Vec<DnsRecord>> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Cloudflare response: {e}"))?;

        if !cf_resp.success {
            let msgs: Vec<String> = cf_resp.errors.iter().map(|e| e.message.clone()).collect();
            return Err(format!("Cloudflare error: {}", msgs.join(", ")));
        }

        cf_resp.result.ok_or_else(|| "No result in Cloudflare response".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_record_deserialize() {
        let json = r#"{"id":"rec-1","name":"app.example.com","type":"A","content":"1.2.3.4","proxied":true}"#;
        let record: DnsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.name, "app.example.com");
        assert_eq!(record.record_type, "A");
        assert!(record.proxied);
    }

    #[test]
    fn test_client_construction() {
        let client = CloudflareClient::new("cf-key", "zone-123");
        assert_eq!(client.api_key, "cf-key");
        assert_eq!(client.zone_id, "zone-123");
    }
}
```

**Step 2: Update deploy/mod.rs**

```rust
pub mod cloudflare;
pub mod commands;
pub mod coolify;
pub mod credentials;
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test deploy::cloudflare`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/deploy/cloudflare.rs src-tauri/src/deploy/mod.rs
git commit -m "feat: add Cloudflare DNS client for subdomain management"
```

---

## Phase 4: Project Type Detection

### Task 11: Project type detection logic

**Files:**
- Create: `src/lib/deploy-detection.ts`
- Create: `src/lib/deploy-detection.test.ts`

**Step 1: Write the failing test**

Create `src/lib/deploy-detection.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { detectProjectType, type ProjectType } from "./deploy-detection";

// Mock the backend command for file existence checks
vi.mock("$lib/backend", () => ({
  command: vi.fn(),
}));

import { command } from "$lib/backend";
const mockCommand = vi.mocked(command);

describe("detectProjectType", () => {
  beforeEach(() => {
    mockCommand.mockReset();
  });

  it("detects static site when vite.config exists without server entry", async () => {
    mockCommand.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === "list_directories_at") return [];
      // Simulate file check responses
      return { has_dockerfile: false, has_package_json: true, has_vite_config: true, has_start_script: false, has_pyproject: false };
    });
    const result = await detectProjectType("/some/path");
    expect(result).toBe("static");
  });

  it("detects node service when package.json has start script", async () => {
    mockCommand.mockResolvedValue({
      has_dockerfile: false, has_package_json: true, has_vite_config: false,
      has_start_script: true, has_pyproject: false,
    });
    const result = await detectProjectType("/some/path");
    expect(result).toBe("node");
  });

  it("detects docker when Dockerfile exists", async () => {
    mockCommand.mockResolvedValue({
      has_dockerfile: true, has_package_json: false, has_vite_config: false,
      has_start_script: false, has_pyproject: false,
    });
    const result = await detectProjectType("/some/path");
    expect(result).toBe("docker");
  });

  it("detects python when pyproject.toml exists", async () => {
    mockCommand.mockResolvedValue({
      has_dockerfile: false, has_package_json: false, has_vite_config: false,
      has_start_script: false, has_pyproject: true,
    });
    const result = await detectProjectType("/some/path");
    expect(result).toBe("python");
  });

  it("returns unknown when no signals match", async () => {
    mockCommand.mockResolvedValue({
      has_dockerfile: false, has_package_json: false, has_vite_config: false,
      has_start_script: false, has_pyproject: false,
    });
    const result = await detectProjectType("/some/path");
    expect(result).toBe("unknown");
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/deploy-detection.test.ts`
Expected: FAIL — module not found

**Step 3: Create deploy-detection.ts**

Create `src/lib/deploy-detection.ts`:

```typescript
import { command } from "$lib/backend";

export type ProjectType = "static" | "node" | "python" | "docker" | "unknown";

export interface ProjectSignals {
  has_dockerfile: boolean;
  has_package_json: boolean;
  has_vite_config: boolean;
  has_start_script: boolean;
  has_pyproject: boolean;
}

export function classifyProject(signals: ProjectSignals): ProjectType {
  // Priority order: static > docker > node > python
  if (signals.has_vite_config && !signals.has_start_script) return "static";
  if (signals.has_dockerfile) return "docker";
  if (signals.has_package_json && signals.has_start_script) return "node";
  if (signals.has_pyproject) return "python";
  return "unknown";
}

export async function detectProjectType(repoPath: string): Promise<ProjectType> {
  const signals = await command<ProjectSignals>("detect_project_type", { repoPath });
  return classifyProject(signals);
}
```

**Step 4: Update tests to use classifyProject directly (pure function, no mock needed)**

Replace the test to test `classifyProject` directly:

```typescript
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
```

**Step 5: Run test**

Run: `npx vitest run src/lib/deploy-detection.test.ts`
Expected: PASS

**Step 6: Add Rust command for file detection**

In `src-tauri/src/deploy/commands.rs`, add:

```rust
#[derive(Serialize)]
pub struct ProjectSignals {
    pub has_dockerfile: bool,
    pub has_package_json: bool,
    pub has_vite_config: bool,
    pub has_start_script: bool,
    pub has_pyproject: bool,
}

#[tauri::command]
pub async fn detect_project_type(repo_path: String) -> Result<ProjectSignals, String> {
    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&repo_path);
        let has_package_json = path.join("package.json").exists();
        let has_start_script = if has_package_json {
            std::fs::read_to_string(path.join("package.json"))
                .map(|content| content.contains("\"start\""))
                .unwrap_or(false)
        } else {
            false
        };

        Ok(ProjectSignals {
            has_dockerfile: path.join("Dockerfile").exists(),
            has_package_json,
            has_vite_config: path.join("vite.config.ts").exists()
                || path.join("vite.config.js").exists()
                || path.join("astro.config.mjs").exists()
                || path.join("next.config.js").exists()
                || path.join("next.config.mjs").exists(),
            has_start_script,
            has_pyproject: path.join("pyproject.toml").exists()
                || path.join("requirements.txt").exists(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
```

Register in `lib.rs`:
```rust
            deploy::commands::detect_project_type,
```

**Step 7: Run all tests**

Run: `npx vitest run src/lib/deploy-detection.test.ts && cd src-tauri && cargo test`
Expected: PASS

**Step 8: Commit**

```bash
git add src/lib/deploy-detection.ts src/lib/deploy-detection.test.ts src-tauri/src/deploy/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add convention-based project type detection"
```

---

## Phase 5: Infrastructure Dashboard UI

### Task 12: Add deployment stores

**Files:**
- Create: `src/lib/deploy-stores.ts`
- Create: `src/lib/deploy-stores.test.ts`

**Step 1: Create stores**

Create `src/lib/deploy-stores.ts`:

```typescript
import { writable } from "svelte/store";

export interface DeployedService {
  uuid: string;
  name: string;
  subdomain: string;
  projectType: "static" | "node" | "python" | "docker";
  status: "running" | "stopped" | "deploying" | "error";
  cpuPercent: number;
  memoryMb: number;
  uptimeSeconds: number;
  lastDeployedAt: string;
  deployTarget: "coolify" | "cloudflare-pages";
}

export const deployedServices = writable<DeployedService[]>([]);
export const selectedServiceId = writable<string | null>(null);
export const serviceLogLines = writable<string[]>([]);
```

**Step 2: Write test**

Create `src/lib/deploy-stores.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { get } from "svelte/store";
import { deployedServices, selectedServiceId } from "./deploy-stores";

describe("deploy stores", () => {
  it("starts with empty services list", () => {
    expect(get(deployedServices)).toEqual([]);
  });

  it("starts with no selected service", () => {
    expect(get(selectedServiceId)).toBeNull();
  });
});
```

**Step 3: Run test**

Run: `npx vitest run src/lib/deploy-stores.test.ts`
Expected: PASS

**Step 4: Commit**

```bash
git add src/lib/deploy-stores.ts src/lib/deploy-stores.test.ts
git commit -m "feat: add deployment state stores"
```

---

### Task 13: Build InfrastructureDashboard with service list

**Files:**
- Modify: `src/lib/InfrastructureDashboard.svelte`
- Modify: `src/lib/InfrastructureDashboard.test.ts`

**Step 1: Write failing test**

Update `src/lib/InfrastructureDashboard.test.ts`:

```typescript
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/svelte";
import { deployedServices, selectedServiceId } from "./deploy-stores";
import InfrastructureDashboard from "./InfrastructureDashboard.svelte";

beforeEach(() => {
  deployedServices.set([]);
  selectedServiceId.set(null);
});

describe("InfrastructureDashboard", () => {
  it("renders empty state when no services deployed", () => {
    render(InfrastructureDashboard);
    expect(screen.getByText(/no services deployed/i)).toBeTruthy();
  });

  it("renders service cards when services exist", () => {
    deployedServices.set([
      {
        uuid: "svc-1",
        name: "myapp",
        subdomain: "myapp.example.com",
        projectType: "node",
        status: "running",
        cpuPercent: 3,
        memoryMb: 128,
        uptimeSeconds: 86400,
        lastDeployedAt: "2026-03-11T00:00:00Z",
        deployTarget: "coolify",
      },
    ]);
    render(InfrastructureDashboard);
    expect(screen.getByText("myapp")).toBeTruthy();
    expect(screen.getByText(/running/i)).toBeTruthy();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/InfrastructureDashboard.test.ts`
Expected: FAIL — service cards not rendered

**Step 3: Update InfrastructureDashboard.svelte**

Replace `src/lib/InfrastructureDashboard.svelte` with the full implementation:

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { deployedServices, selectedServiceId, serviceLogLines, type DeployedService } from "./deploy-stores";
  import { hotkeyAction } from "./stores";
  import { showToast } from "./toast";

  const servicesState = fromStore(deployedServices);
  const selectedState = fromStore(selectedServiceId);
  const logLinesState = fromStore(serviceLogLines);

  let services = $derived(servicesState.current);
  let selectedId = $derived(selectedState.current);
  let logs = $derived(logLinesState.current);

  let selectedService = $derived(services.find(s => s.uuid === selectedId) ?? null);

  function formatUptime(seconds: number): string {
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
    return `${Math.floor(seconds / 86400)}d`;
  }

  function statusColor(status: string): string {
    switch (status) {
      case "running": return "#a6e3a1";
      case "stopped": return "#6c7086";
      case "deploying": return "#89b4fa";
      case "error": return "#f38ba8";
      default: return "#a6adc8";
    }
  }
</script>

<div class="container">
  {#if services.length === 0}
    <div class="empty-state">
      <div class="title">Infrastructure</div>
      <div class="subtitle">No services deployed yet</div>
      <div class="hint">Deploy a project with <kbd>d</kbd> from the infrastructure workspace</div>
    </div>
  {:else}
    <div class="dashboard">
      <div class="service-list">
        {#each services as service}
          <button
            class="service-card"
            class:selected={selectedId === service.uuid}
            onclick={() => selectedServiceId.set(service.uuid)}
          >
            <div class="service-header">
              <span class="status-dot" style="background: {statusColor(service.status)}"></span>
              <span class="service-name">{service.name}</span>
              <span class="service-status">{service.status}</span>
            </div>
            <div class="service-meta">
              {#if service.deployTarget === "cloudflare-pages"}
                <span class="meta-item">Cloudflare Pages</span>
              {:else}
                <span class="meta-item">CPU: {service.cpuPercent}%</span>
                <span class="meta-item">RAM: {service.memoryMb}MB</span>
                <span class="meta-item">{formatUptime(service.uptimeSeconds)} uptime</span>
              {/if}
            </div>
          </button>
        {/each}
      </div>

      <div class="log-panel">
        <div class="log-header">
          {#if selectedService}
            Logs — {selectedService.name}
          {:else}
            Select a service to view logs
          {/if}
        </div>
        <div class="log-content">
          {#each logs as line}
            <div class="log-line">{line}</div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .container {
    height: 100%;
    background: #11111b;
    color: #cdd6f4;
    display: flex;
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .title { font-size: 18px; font-weight: 600; margin-bottom: 8px; }
  .subtitle { font-size: 14px; color: #a6adc8; margin-bottom: 16px; }
  .hint { font-size: 12px; color: #6c7086; }
  kbd { background: #313244; padding: 2px 6px; border-radius: 3px; font-family: monospace; font-size: 11px; }

  .dashboard {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 16px;
    gap: 12px;
  }

  .service-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .service-card {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 6px;
    padding: 12px 16px;
    cursor: pointer;
    text-align: left;
    color: #cdd6f4;
    font-family: inherit;
    font-size: 13px;
  }

  .service-card:hover { border-color: #45475a; }
  .service-card.selected { border-color: #89b4fa; background: rgba(137, 180, 250, 0.05); }

  .service-header { display: flex; align-items: center; gap: 8px; }
  .status-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .service-name { font-weight: 600; flex: 1; }
  .service-status { font-size: 12px; color: #a6adc8; }

  .service-meta { display: flex; gap: 12px; margin-top: 6px; font-size: 11px; color: #6c7086; }
  .meta-item {}

  .log-panel {
    flex: 1;
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    min-height: 200px;
  }

  .log-header {
    padding: 8px 12px;
    border-bottom: 1px solid #313244;
    font-size: 12px;
    color: #a6adc8;
  }

  .log-content {
    flex: 1;
    padding: 8px 12px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 12px;
    color: #a6adc8;
  }

  .log-line { padding: 1px 0; }
</style>
```

**Step 4: Run tests**

Run: `npx vitest run src/lib/InfrastructureDashboard.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/InfrastructureDashboard.svelte src/lib/InfrastructureDashboard.test.ts
git commit -m "feat: build InfrastructureDashboard with service list and log panel"
```

---

## Phase 6: Deploy Flow Integration

### Task 14: Add deploy Tauri commands (wiring Coolify + Cloudflare)

**Files:**
- Modify: `src-tauri/src/deploy/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add deploy_project command**

In `src-tauri/src/deploy/commands.rs`, add:

```rust
use super::coolify::CoolifyClient;
use super::cloudflare::CloudflareClient;

#[derive(Deserialize)]
pub struct DeployRequest {
    pub project_name: String,
    pub repo_path: String,
    pub subdomain: String,
    pub project_type: String,
}

#[derive(Serialize)]
pub struct DeployResult {
    pub url: String,
    pub coolify_uuid: String,
}

#[tauri::command]
pub async fn deploy_project(request: DeployRequest) -> Result<DeployResult, String> {
    let creds = DeployCredentials::load()?;
    if !creds.is_provisioned() {
        return Err("Deploy not provisioned. Run setup first.".to_string());
    }

    let coolify = CoolifyClient::new(
        creds.coolify_url.as_ref().unwrap(),
        creds.coolify_api_key.as_ref().unwrap(),
    );

    // Check if app already exists in Coolify
    let apps = coolify.list_applications().await?;
    let existing = apps.iter().find(|a| a.name == request.project_name);

    let uuid = if let Some(app) = existing {
        // Redeploy existing app
        coolify.deploy_application(&app.uuid).await?;
        app.uuid.clone()
    } else {
        // TODO: Create new application in Coolify via API
        // For now, return error until Coolify app creation is implemented
        return Err("Creating new Coolify applications not yet implemented. Create the app in Coolify UI first.".to_string());
    };

    let domain = format!("{}.{}", request.subdomain, creds.root_domain.unwrap());
    let url = format!("https://{domain}");

    Ok(DeployResult { url, coolify_uuid: uuid })
}

#[tauri::command]
pub async fn list_deployed_services() -> Result<Vec<serde_json::Value>, String> {
    let creds = DeployCredentials::load()?;
    if !creds.is_provisioned() {
        return Ok(vec![]);
    }

    let coolify = CoolifyClient::new(
        creds.coolify_url.as_ref().unwrap(),
        creds.coolify_api_key.as_ref().unwrap(),
    );

    let apps = coolify.list_applications().await?;
    let result: Vec<serde_json::Value> = apps
        .iter()
        .map(|app| {
            serde_json::json!({
                "uuid": app.uuid,
                "name": app.name,
                "status": app.status,
                "fqdn": app.fqdn,
            })
        })
        .collect();

    Ok(result)
}
```

**Step 2: Register commands in lib.rs**

Add to `generate_handler![]`:

```rust
            deploy::commands::deploy_project,
            deploy::commands::list_deployed_services,
```

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: PASS (compiles, existing tests pass)

**Step 4: Commit**

```bash
git add src-tauri/src/deploy/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add deploy_project and list_deployed_services Tauri commands"
```

---

### Task 15: Wire deploy hotkey to backend

**Files:**
- Modify: `src/App.svelte`

**Step 1: Update the deploy hotkey handler in App.svelte**

Replace the TODO placeholder in the `deploy-project` handler:

```typescript
      } else if (action?.type === "deploy-project") {
        command<boolean>("is_deploy_provisioned").then(async (provisioned) => {
          if (!provisioned) {
            deploySetupOpen = true;
          } else {
            try {
              showToast("Deploying...", "info");
              const result = await command<{ url: string; coolify_uuid: string }>("deploy_project", {
                request: {
                  project_name: action.projectId, // Will be refined with actual project name
                  repo_path: action.repoPath,
                  subdomain: action.projectId.substring(0, 8),
                  project_type: "node",
                },
              });
              showToast(`Deployed to ${result.url}`, "info");
            } catch (e) {
              showToast(String(e), "error");
            }
          }
        });
```

**Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: PASS

**Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat: wire deploy hotkey to Coolify backend"
```

---

## Phase 7: Sidebar Navigation for Infrastructure Mode

### Task 16: Update HotkeyManager sidebar navigation for infrastructure mode

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:142-175`

**Step 1: Write the failing test**

Add test in `src/lib/HotkeyManager.test.ts`:

```typescript
it("j/k navigates projects in infrastructure mode", () => {
  workspaceMode.set("infrastructure");
  pressKey("j");
  // Verify focusTarget changed to next project
  expect(get(focusTarget)).toEqual({ type: "project", projectId: expect.any(String) });
});
```

**Step 2: Update getVisibleItems()**

In `src/lib/HotkeyManager.svelte`, update the `getVisibleItems()` function. Add a case for infrastructure mode (projects only, no sub-items):

```typescript
    if (currentMode === "infrastructure") {
      const result: SidebarItem[] = [];
      for (const p of projectList) {
        result.push({ type: "project", projectId: p.id });
      }
      return result;
    }
```

Add this block at the beginning of the function, after the `agents` check.

**Step 3: Run test**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 4: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "feat: add infrastructure mode sidebar navigation"
```

---

## Summary

This plan covers the full foundation:

| Phase | Tasks | What it delivers |
|-------|-------|-----------------|
| 1 | 1-4 | Workspace shell — mode, picker, empty dashboard, hotkeys |
| 2 | 5-7 | Credential storage + onboarding modal |
| 3 | 8-10 | Coolify + Cloudflare API clients |
| 4 | 11 | Project type detection |
| 5 | 12-13 | Infrastructure dashboard UI with service list + logs |
| 6 | 14-15 | Deploy flow wiring (hotkey → Coolify → notification) |
| 7 | 16 | Sidebar navigation in infrastructure mode |

**Not included (future phases):**
- Hetzner server provisioning (auto-create VPS via API)
- Coolify auto-installation on the VPS
- Static site deployment to Cloudflare Pages
- Log streaming via WebSocket/SSE
- Real-time metrics polling
- Rollback execution
- First-deploy modal with subdomain + secrets configuration
- Litestream SQLite backup setup

These are deferred because they depend on having a real Coolify instance to test against. The foundation above can be built and tested with mocks, then the API integrations can be validated end-to-end once infrastructure is provisioned.
