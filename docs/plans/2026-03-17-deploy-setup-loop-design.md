# Deploy Setup Loop Design

## Definition

Fix issue #18: the deploy setup wizard currently saves credentials that can never satisfy `DeployCredentials::is_provisioned()`, so every deploy attempt reopens setup instead of advancing to deployment.

Recommended approach:

1. Keep the backend provisioning gate strict.
2. Update the frontend setup wizard to collect and persist every field required by that gate.
3. Add a frontend regression test that completes setup, re-triggers deploy, and proves the flow reaches `deploy_project`.

Alternative approaches considered:

1. Relax `is_provisioned()` to match the current modal.
   - Rejected because `deploy_project` still requires Coolify URL, Coolify API key, and server IP. This only moves the failure deeper into the flow.
2. Implement real Hetzner/Coolify/Cloudflare provisioning from the wizard.
   - Rejected for this issue because there is no existing provisioning backend and that scope is much larger than the reported regression.

## Constraints

- Follow the existing deploy architecture: deployment still depends on stored Coolify and server details.
- Keep the change minimal and local to the deploy setup flow unless evidence shows backend logic is wrong.
- Use TDD: write a failing regression test first and verify it fails for the current bug.
- Preserve current modal invocation from `src/App.svelte`; avoid broad deploy UX changes beyond removing the dead-end.
- Maintain repository process requirements: verify with `pnpm check`, `cargo fmt --check`, and `cargo clippy -- -D warnings` after code changes.

## Validation

- Add an `App` regression test that:
  - triggers `deploy-project`
  - completes the setup modal with full manual deploy details
  - triggers `deploy-project` again
  - verifies the second attempt reaches `command("deploy_project", ...)`
- Make the test stateful so setup only counts as provisioned when the saved credentials include all required fields.
- Run the new test first and confirm it fails on the current implementation.
- After the fix, rerun the focused test and confirm it passes.
- As a regression check, reverting the implementation should make the new test fail again because the saved credential payload would no longer mark deploy as provisioned.
