# Deploy Setup Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Make deploy setup persist a provisionable credential set so the deploy flow can advance after setup instead of reopening the wizard forever.

**Architecture:** Keep backend provisioning rules unchanged. Fix the dead-end in the frontend by collecting the missing manual Coolify/server fields in `DeploySetupModal`, then cover the end-to-end UI flow with a stateful `App` regression test that proves the second deploy attempt reaches the backend deploy command.

**Tech Stack:** Svelte 5, Vitest, Testing Library, Tauri backend commands

---

### Task 1: Add the failing regression test

**Files:**
- Modify: `src/App.test.ts`

**Step 1: Write the failing test**

Add a test that:
- mocks `is_deploy_provisioned` from saved in-memory credentials
- opens deploy setup on the first deploy attempt
- completes the modal with Hetzner, Cloudflare, root domain, Coolify URL, Coolify API key, and server IP
- triggers deploy again
- expects `deploy_project` to be called

**Step 2: Run test to verify it fails**

Run: `pnpm test -- src/App.test.ts -t "reaches deploy_project after completing deploy setup"`

Expected: FAIL because the current modal never saves the fields required to mark deploy as provisioned.

### Task 2: Fix the deploy setup wizard

**Files:**
- Modify: `src/lib/DeploySetupModal.svelte`

**Step 1: Write minimal implementation**

- Add inputs for Coolify URL, Coolify API key, and server IP.
- Save those fields in the `save_deploy_credentials` payload.
- Replace the fake provisioning step with a final manual configuration step that matches the backend contract.

**Step 2: Run focused test to verify it passes**

Run: `pnpm test -- src/App.test.ts -t "reaches deploy_project after completing deploy setup"`

Expected: PASS

### Task 3: Add focused modal coverage

**Files:**
- Modify: `src/lib/DeploySetupModal.test.ts`

**Step 1: Add a focused payload test**

Verify the modal submits `save_deploy_credentials` with non-null Coolify URL, Coolify API key, and server IP values.

**Step 2: Run modal tests**

Run: `pnpm test -- src/lib/DeploySetupModal.test.ts`

Expected: PASS

### Task 4: Run verification gates and finish branch

**Files:**
- Verify only: `src/App.test.ts`
- Verify only: `src/lib/DeploySetupModal.svelte`
- Verify only: `src/lib/DeploySetupModal.test.ts`
- Verify only: `docs/plans/2026-03-17-deploy-setup-loop-design.md`
- Verify only: `docs/plans/2026-03-17-deploy-setup-loop.md`

**Step 1: Run project verification**

Run:
- `pnpm test -- src/App.test.ts -t "reaches deploy_project after completing deploy setup"`
- `pnpm test -- src/lib/DeploySetupModal.test.ts`
- `pnpm check`
- `cd src-tauri && cargo fmt --check`
- `cd src-tauri && cargo clippy -- -D warnings`

**Step 2: Commit**

Use a conventional commit message that includes `closes #18` and the trailer `Contributed-by: auto-worker`.
