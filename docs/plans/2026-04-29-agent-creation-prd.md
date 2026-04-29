# PRD: Agent Creation Mode

Date: 2026-04-29
Status: Draft

Related docs:

- `docs/plans/2026-04-29-controller-agent-product-prd.md`
- `docs/plans/2026-04-29-chat-routing-prd.md`
- `docs/plans/2026-04-27-controller-chat-modes-daemon-rfc.md`
- `docs/keyboard-modes.md`

## Table of Contents

- [Summary](#summary)
- [Design Assets](#design-assets)
- [Problem](#problem)
- [Goals](#goals)
- [Non-goals](#non-goals)
- [Users](#users)
- [Use Cases](#use-cases)
- [User Stories](#user-stories)
- [Product Principles](#product-principles)
- [Frontend UX Requirements](#frontend-ux-requirements)
- [Frontend UI Requirements](#frontend-ui-requirements)
- [Backend and Daemon Requirements](#backend-and-daemon-requirements)
- [Validation and Error States](#validation-and-error-states)
- [Lifecycle Rules](#lifecycle-rules)
- [Acceptance Criteria](#acceptance-criteria)
- [Open Questions](#open-questions)
- [Proposed Phases](#proposed-phases)

## Summary

Agent Creation Mode lets a developer create and maintain reusable agent profiles
through a focused list-and-editor interface, then test a saved profile by
launching it into chat.

The v1 product shape is deliberately simple:

- a profile list;
- a profile editor;
- a collapsible preview drawer;
- a `Test in Chat` action;
- a generated pixel-art avatar for each saved profile.

Agent Creation Mode does not generate profiles for the user. The user writes
the name, handle, prompt, skills, workspace defaults, and outbox instructions.
The product validates that definition, saves it as a durable profile version,
and makes the profile available to chat routing and workflow creation.

## Design Assets

- Simplified Agent Creation Mode mockup:
  `docs/assets/design/agent-creation-mode-simplified.png`

## Problem

The Controller is moving from terminal-session orchestration to agent
orchestration. Chat routing depends on configured agent profiles, but those
profiles need a first-class place to exist.

Without Agent Creation Mode, users must manage reusable specialists through
daemon files, future backend APIs, or ad hoc setup flows. That makes agents hard
to discover, hard to test, and hard to trust. It also weakens the meaning of
`@agent` and `%agent`, because the user cannot see what an agent handle maps to
before routing work to it.

Agent Creation Mode should give the user one durable place to answer:

- Which reusable agents exist?
- Which handle do I type in chat?
- Which runtime, model, prompt, and skills does this agent use?
- What context will the daemon materialize before launch?
- Will this profile validate before I use it?

## Goals

1. Add a dedicated workspace mode for manual agent profile creation.
2. Keep the UI compact: profile list, editor, preview drawer, and test action.
3. Let users create, edit, duplicate, archive, and restore profiles.
4. Validate handles used by `@agent` and `%agent`.
5. Configure runtime, model, prompt, skills, workspace behavior, and outbox
   instructions.
6. Preview generated runtime context before launch.
7. Save profile versions so existing sessions remain explainable.
8. Make saved profiles available to chat suggestions and workflow builders.
9. Launch a smoke-test chat from a saved profile without embedding chat in the
   editor.
10. Generate a 256 by 256 pixel-art avatar for each new profile in the
    background.

## Non-goals

- Agent-assisted profile generation.
- A live embedded transcript inside Agent Creation Mode.
- Workflow authoring.
- Agent observability beyond profile-level metadata and validation.
- Arbitrary third-party runtime plugins.
- Editing already-running sessions by changing their source profile.
- Full prompt library management.
- Manual avatar drawing in v1.
- Multi-user approval or publishing workflows.

## Users

### Interactive Developer

Uses the Controller as a daily workspace and wants a small set of reusable
specialists, such as `@reviewer`, `@planner`, and `%debugger`.

### Agent Orchestrator

Coordinates agents across chats and future workflows. They need stable handles,
clear ownership semantics, and confidence that each profile launches with the
expected prompt and skills.

### Prompt Maintainer

Tunes agent instructions over time. They need profile versions, validation, and
a preview of generated context so they can explain why a session behaved a
certain way.

## Use Cases

### Create a Reusable Reviewer

The user opens Agent Creation Mode, presses `n`, names the profile `Reviewer`,
sets handle `reviewer`, chooses a runtime and model, writes the system prompt,
adds code-review skills, and saves. The saved profile appears as `@reviewer` in
chat suggestions. The Controller starts a background avatar job for a 256 by
256 pixel-art reviewer character.

### Create a Shadow Debugger Profile

The user creates `Debugger` with handle `debugger`, focused troubleshooting
instructions, and a default workspace behavior. In chat, the profile appears as
`%debugger` for temporary shadow sessions and `@debugger` when reusable routing
is appropriate.

### Preview Runtime Context

Before saving, the user opens the preview drawer. The Controller shows the
generated prompt shape, selected skills, workspace-context rules, and outbox
instructions that the daemon will materialize for future sessions.

### Fix a Validation Error

The user enters handle `reviewer!`. The editor marks the handle invalid and
disables saving until the user changes it to `reviewer`.

### Duplicate a Profile

The user duplicates `Reviewer` into `Security Reviewer`, edits the prompt and
skills, and saves it with handle `security-reviewer`. Existing sessions launched
from `Reviewer` continue to reference the older profile version.

### Smoke-Test a Saved Profile

The user clicks `Test in Chat`. The Controller creates a temporary chat for the
saved profile and focuses the chat composer. Agent Creation Mode remains a
profile editor; chat remains the runtime surface.

### Generate a Profile Avatar

After the user saves a new profile, the Controller launches a background Codex
CLI job that invokes the image generation tool. The prompt uses the agent name
and role to create a 256 by 256 pixel-art humanoid avatar. The avatar may be an
elf, human, android, or another humanoid form that fits the role. The profile
uses an initials avatar until generation succeeds.

## User Stories

- As a developer, I want a dedicated Agent Creation Mode so reusable agents are
  not hidden inside chat setup.
- As a developer, I want to see all profiles in one list so I can find,
  duplicate, archive, or edit an agent quickly.
- As a developer, I want to configure a stable handle so I can type `@reviewer`
  or `%debugger` in chat.
- As a developer, I want handle validation before save so broken handles do not
  appear in routing suggestions.
- As a developer, I want to choose runtime and model so each agent uses the
  right execution path.
- As a developer, I want to write the system prompt directly so I control the
  agent's behavior.
- As a developer, I want to choose skills so the daemon materializes the right
  capabilities for the runtime.
- As a developer, I want to configure workspace behavior so an agent knows
  whether it expects an edit target.
- As a developer, I want outbox instructions so chat transcripts show
  intentional replies instead of raw runtime output.
- As a developer, I want to preview generated context so I understand what the
  agent will see at launch.
- As a developer, I want saved profile versions so old sessions stay
  explainable after I edit a profile.
- As a developer, I want `Test in Chat` so I can try a saved profile in the real
  chat surface.
- As a developer, I want each saved profile to get a generated avatar so agents
  feel distinct in lists, chat tokens, and observability.

## Product Principles

### Creation Is a Workspace, Not a Modal

The user should be able to manage profiles as durable product objects. Agent
Creation Mode gets its own workspace mode rather than living as a secondary
dialog inside chat.

### Manual Means Explicit

The product should not write prompts for the user in v1. It should help the
user see validation, context preview, and routing consequences while preserving
direct control over the profile.

### Avatars Add Identity Without Blocking Work

Avatar generation should happen after save and should not block profile use. If
generation fails, the profile should remain valid with an initials avatar and a
retry action.

### The Editor Stays Small

The v1 editor should not become a playground. Profile creation needs one form,
one preview drawer, and one smoke-test action.

### Chat Owns Runtime Interaction

Agent Creation Mode defines profiles. Chat runs them. `Test in Chat` should
launch the user into chat instead of embedding a transcript in the editor.

### Versions Explain Behavior

Profile edits should affect future launches. Running or historical sessions
should keep the profile version that launched them.

## Frontend UX Requirements

### Mode Entry

- Agent Creation Mode is a distinct workspace mode.
- The mode switcher should expose Agent Creation without slowing the current
  chat flow.
- Opening the mode shows the profile list and the selected profile editor.
- If no profiles exist, the mode shows an empty state with a primary `New
  Profile` action.

### Profile List

- The list shows active profiles by default.
- Each row shows avatar, profile name, handle, runtime, and validation/status
  state.
- The list supports search by name, handle, runtime, and skill.
- The list supports filters for active and archived profiles.
- The list supports keyboard navigation.
- Pressing `n` in Agent Creation Mode starts a new draft profile when no text
  input is focused.
- Selecting a row opens that profile in the editor.

### Profile Editor

The editor includes:

- generated avatar;
- profile name;
- handle;
- runtime;
- model or provider option when applicable;
- short description;
- system prompt;
- skills;
- default workspace behavior;
- outbox instructions.

The editor should autoshow unsaved state. Save remains explicit.

### Preview Drawer

- The preview drawer is collapsed by default.
- The user can open the drawer from the editor header.
- The drawer shows generated runtime context, selected skills, workspace
  behavior, and outbox instructions.
- The drawer opens automatically when validation fails on generated context.
- The drawer should not compete with the main editor when everything validates.

### Profile Actions

- `Save` creates a new profile version.
- `Duplicate` creates a draft from the selected profile.
- `Archive` removes the profile from default suggestions without deleting
  historical session references.
- `Restore` makes an archived profile active again.
- `Test in Chat` is enabled only for a saved valid profile.
- `Regenerate Avatar` starts a new background avatar job for a saved profile.

### Avatar Generation

- New profiles start with an initials avatar.
- After first save, the Controller starts avatar generation in the background.
- Avatar generation uses Codex CLI to run an image generation task.
- The generated image must be pixel art.
- The generated image must be 256 by 256 pixels.
- The prompt must use the agent name and role.
- The subject should be humanoid, such as an elf, human, android, or similar
  character type.
- Avatar generation must not block saving, editing, or `Test in Chat`.
- If generation fails, the UI keeps the initials avatar and exposes retry.
- Regenerating an avatar should not create a new profile version unless profile
  fields also changed.

### Test in Chat

- `Test in Chat` creates a temporary chat or profile-backed chat session.
- The new chat uses the saved profile version.
- The Controller focuses the chat composer after creation.
- The originating profile remains linked so the user can return to edit it.
- Unsaved edits do not affect the smoke-test session.

## Frontend UI Requirements

### Layout

Agent Creation Mode uses three surfaces:

1. Profile list on the left.
2. Profile editor in the main panel.
3. Collapsible preview drawer on the right.

The preview drawer should be hidden until needed. In the common case, the user
sees a simple list-and-editor layout.

### Information Hierarchy

The editor header should show:

- avatar;
- profile name;
- handle;
- runtime;
- current version;
- save state;
- primary actions.

The first visible editor fields should be name, handle, runtime, and model.
Prompt and outbox instructions can use taller text areas below those fields.
Skills and workspace behavior should remain compact.

### Visual Treatment

- Use the existing Catppuccin Mocha visual language.
- Use the same profile card and prompt preview vocabulary as the component
  board in `docs/assets/design/controller-agent-ui-component-board.png`.
- Keep cards to repeated list rows or framed previews.
- Avoid nested cards.
- Use compact controls and icon buttons where the action is familiar.
- Use validation colors sparingly: green for valid, yellow for warning, red for
  blocking error.

### Empty State

When no profiles exist, show:

- title: `No agent profiles`;
- primary action: `New Profile`;
- short supporting text that tells the user profiles become available as
  `@agent` and `%agent` in chat.

### Dirty State

When the editor has unsaved changes:

- show an unsaved indicator in the header;
- keep `Test in Chat` disabled or point it at the last saved version with clear
  copy;
- warn before navigating away if changes would be lost.

### Archived State

Archived profiles:

- do not appear in default chat suggestions;
- can appear in Agent Creation Mode when the archived filter is enabled;
- can still be referenced by historical sessions;
- can be restored.

## Backend and Daemon Requirements

The Controller backend and daemon need durable records for:

- agent profile id;
- profile version id;
- avatar image path or asset id;
- avatar generation status;
- avatar generation error;
- name;
- handle;
- runtime;
- model or provider options;
- description;
- system prompt;
- skills;
- default workspace behavior;
- outbox instructions;
- archive state;
- timestamps;
- validation state or last validation result.

The backend should expose browser-safe endpoints for profile CRUD and validation.
The daemon should own runtime-specific materialization and launch behavior. The
Controller should schedule avatar jobs separately from profile materialization
so profile launch does not depend on image generation.

### Avatar Jobs

- Avatar jobs run in the background after profile creation.
- Avatar jobs use Codex CLI with access to the image generation tool.
- The job prompt includes profile name, role or description, and required
  output shape: 256 by 256 pixel-art humanoid avatar.
- The job stores the generated image as a local asset and records the asset id
  or path on the profile.
- The job records pending, succeeded, and failed states.
- Failed jobs should not mark the profile invalid.
- Regeneration replaces the current avatar only after a new image succeeds.

### Profile Versions

- Every successful save creates a new profile version.
- Sessions store the profile version id used at launch.
- Editing a profile does not mutate sessions that already exist.
- Archived profiles keep their historical versions.
- Workflow definitions should reference stable profile ids and may pin profile
  versions later if workflow reproducibility requires it.

### Handle Semantics

- Handles must be unique among active profiles.
- Handles must be stable identifiers for chat suggestions.
- Handles should support lowercase letters, numbers, and hyphens.
- Handles should not include the `@` or `%` prefix in storage.
- Chat rendering adds `@` or `%` based on routing semantics.

### Runtime Materialization

When a profile launches, the daemon materializes:

- runtime invocation settings;
- system prompt;
- selected skills;
- workspace-context instructions;
- outbox instructions;
- profile id and version metadata.

The daemon records launch events with enough metadata for Agent Observability
Mode to show which profile version produced the session.

## Validation and Error States

### Blocking Validation

Save is blocked when:

- name is empty;
- handle is empty;
- handle contains unsupported characters;
- handle collides with another active profile;
- runtime is unavailable;
- required runtime model/provider settings are missing;
- system prompt is empty;
- selected skills cannot be resolved.

Avatar generation never blocks save.

### Warnings

Save can proceed with warnings when:

- no skills are selected;
- no outbox instructions are provided;
- workspace behavior is unset;
- the prompt is unusually short;
- an archived profile has the same handle.

### Runtime Errors

If `Test in Chat` fails:

- keep the user in context;
- show the failure near the action;
- preserve the saved profile;
- link to any daemon event or log record that explains the failure.

## Lifecycle Rules

### Create

1. User starts a draft profile.
2. User fills required fields.
3. Controller validates fields.
4. User saves.
5. Backend creates profile and first profile version.
6. Profile appears in chat suggestions and workflow profile pickers.
7. Controller starts the background avatar generation job.

### Edit

1. User edits an existing profile.
2. Controller marks the editor dirty.
3. User saves.
4. Backend creates a new profile version.
5. Future launches use the new version.
6. Existing sessions remain tied to their launch version.

### Duplicate

1. User duplicates an existing profile.
2. Controller creates a draft with copied fields.
3. User changes name and handle.
4. Save creates a separate profile id and first version.
5. Controller starts a new avatar generation job for the duplicated profile.

### Archive

1. User archives a profile.
2. Profile leaves default suggestions.
3. Historical sessions remain readable.
4. User can restore the profile later.

### Test

1. User selects a saved valid profile.
2. User chooses `Test in Chat`.
3. Controller requests a profile-backed chat using the saved profile version.
4. Chat mode opens and focuses the composer.
5. Observability can show the launched session with profile metadata.

## Acceptance Criteria

- Agent Creation Mode exists as a distinct workspace mode.
- Users can create, edit, duplicate, archive, restore, and search profiles.
- Users can configure name, handle, runtime, model/provider options, prompt,
  skills, workspace behavior, and outbox instructions.
- Handles are validated before save and stored without `@` or `%`.
- Saves create profile versions.
- Sessions launched from a profile store the profile version id.
- Saved active profiles appear in `@agent` and `%agent` suggestions.
- Archived profiles do not appear in default chat suggestions.
- The preview drawer shows generated context and validation details.
- `Test in Chat` launches a chat from the saved valid profile and does not use
  unsaved edits.
- Saving a new profile starts a background 256 by 256 pixel-art avatar
  generation job using Codex CLI and the image generation tool.
- Avatar generation does not block profile save, profile edit, or chat launch.
- Existing sessions remain explainable after a profile is edited or archived.

## Open Questions

1. Which shortcut should open Agent Creation Mode from the workspace mode
   picker?
2. Should the model field be runtime-specific in v1 or a freeform advanced
   field?
3. Should `Test in Chat` create an automatically named temporary chat or ask for
   a chat name?
4. Should duplicate profiles copy archived state, or should duplicates always
   start active?
5. Should profile versions show a visible changelog in v1, or only a version
   number?
6. Where should generated avatar assets live: Controller project storage,
   daemon storage, or a shared local asset directory?
7. Should archived profiles keep their last generated avatar visible in archive
   views?

## Proposed Phases

### Phase 1: Profile CRUD

- Add Agent Creation Mode.
- Add profile list, empty state, search, and active/archived filters.
- Add editor fields and save.
- Add profile version creation on save.

### Phase 2: Validation and Preview

- Add handle, runtime, prompt, and skill validation.
- Add collapsible generated-context preview drawer.
- Add dirty-state and navigation warnings.
- Add background avatar generation, retry, and initials fallback.

### Phase 3: Chat Integration

- Feed active saved profiles into `@agent` and `%agent` suggestions.
- Add `Test in Chat`.
- Store profile version ids on launched sessions.

### Phase 4: Workflow and Observability Integration

- Feed profiles into workflow builders.
- Show profile id and version metadata in Agent Observability Mode.
- Add restore behavior for archived profiles referenced by historical sessions.
