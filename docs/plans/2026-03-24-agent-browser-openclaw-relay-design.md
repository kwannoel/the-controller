# Agent Browser OpenClaw Relay Design

## Summary

Replace the old `agent-browser` skill with an OpenClaw-style browser workflow that controls an existing Chrome or Chromium tab through an extension relay. The new skill should treat "reuse the browser you already have open and logged into" as the default path. It should keep a managed-browser fallback for cases where the relay is unavailable.

## Problem

The current browser skill model pushes agents toward fresh browser sessions, saved state files, or dedicated automation profiles. That works, but it still creates login churn. Users want the agent to work against the Chrome tab they already use so sessions, cookies, extensions, and in-progress workflows stay intact.

Firefox looked attractive at first, but OpenClaw's existing-browser control model depends on a Chromium relay path. That path matches the user goal better than trying to stretch Playwright Firefox into an unsupported attachment model.

## Goals

- Make existing-browser reuse the primary `agent-browser` workflow.
- Match the OpenClaw mental model closely enough that the setup feels familiar.
- Preserve a fallback path for isolated automation when the user cannot or should not expose their real browser.
- Keep the skill practical for agents: clear setup, clear command sequence, clear failure modes.

## Non-Goals

- Build an actual browser relay service in this repo.
- Implement Firefox attachment through plain Playwright.
- Preserve every old `agent-browser` example or concept if it conflicts with the new primary workflow.

## User Experience

The skill should teach this order:

1. Install and enable the browser relay extension.
2. Open the target site in the user's normal Chrome or Chromium browser.
3. Click the extension on the tab to attach relay control.
4. Run `agent-browser` commands against that attached tab.
5. Detach when done.

If the relay path is not available, the skill should point the agent to a managed Chromium fallback that uses a dedicated browser profile.

## Proposed Skill Shape

### 1. Rewrite the skill around relay-first control

The `agent-browser` skill should stop presenting "launch a fresh browser and automate it" as the first mental model. The new top section should explain that the skill controls an existing Chromium tab through a local relay, following the OpenClaw pattern.

The setup section should cover:

- extension installation
- how tab attachment works
- how the local relay endpoint is discovered or configured
- the security tradeoff of giving a local relay control over a logged-in browser tab

### 2. Keep command workflows, but frame them as operating on the attached tab

The old skill already has a useful command vocabulary and workflow structure:

- `open`
- `snapshot -i`
- `click`
- `fill`
- `wait`
- `screenshot`

That interaction model should stay. The docs should change the browser lifecycle story around it:

- agents operate on the attached existing tab by default
- agents only use a managed browser when the relay path is not active

### 3. Replace the authentication section

The old authentication section focuses on state files, sessions, profiles, and auth vaults. The replacement should reorder those ideas:

- **Primary:** existing logged-in browser tab through relay
- **Secondary:** managed dedicated profile for repeated isolated automation
- **Tertiary:** imported or saved state for edge cases

This keeps the user-facing promise aligned with the actual motivation for the change.

### 4. Add explicit mode selection language

The skill should define two modes:

- `relay mode`: control the user's existing Chrome tab
- `managed mode`: launch or reuse a dedicated automation browser

The docs should tell the agent when to choose each mode.

Use `relay mode` when:

- the user is already logged in
- the task depends on real browser extensions, saved sessions, or account state
- the user wants to avoid reauthentication

Use `managed mode` when:

- the task needs isolation
- the user does not want the agent touching their real browser
- the relay is unavailable or unsupported

## Architecture

This is a documentation and skill-structure change, not a runtime feature in the app. The implementation should update the skill text and any helper templates or references so the skill now describes an OpenClaw-style relay architecture.

The final skill should describe these moving parts:

- an existing Chrome or Chromium browser window
- a browser extension that opts a tab into agent control
- a local relay endpoint that exposes the attached tab
- the `agent-browser` command set that works against that attached tab

## Error Handling

The skill should document the main failure cases:

- extension installed but not attached to the tab
- relay endpoint not running or not reachable
- wrong browser family, such as Firefox or Safari
- stale refs after navigation
- the user attached the wrong tab

Each failure should have a direct recovery step.

## Security Notes

The new skill must state that relay mode gives automation access to a real logged-in browser tab. It should tell the agent to:

- use the minimum tab scope needed
- avoid unrelated tabs
- detach after finishing
- prefer managed mode when handling sensitive environments that should stay isolated

## Testing Strategy

This change is documentation-first, so validation should focus on content correctness and navigability:

- verify the updated skill puts relay mode before all other auth/session reuse paths
- verify the setup sequence is complete and consistent with OpenClaw's documented relay model
- verify fallback guidance exists for managed mode
- verify the skill no longer implies Firefox is the preferred existing-browser path

## Open Questions Resolved

- **Should Firefox remain the target?** No. Existing-browser reuse should follow the Chromium relay path.
- **Should the old `agent-browser` skill survive beside the new model?** No. Replace it with the relay-first model.
- **Should fallback guidance remain?** Yes. Keep a managed-browser fallback for isolation and unsupported environments.
