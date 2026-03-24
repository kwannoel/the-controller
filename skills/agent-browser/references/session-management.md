# Session Management

`agent-browser` supports two session models:

- relay mode: operate on the user's attached live browser tab
- managed mode: operate on a dedicated automation browser or profile

## Relay mode

Relay mode does not create a separate browser session. It reuses the browser state that already exists in the attached Chrome or Chromium tab.

That means:

- cookies and local storage already exist
- browser extensions already exist
- the user can keep interacting with the same account and session

Use relay mode when the user says some version of "use the browser I already have open."

## Managed mode

Managed mode creates or restores browser state that belongs to automation.

Use managed mode when:

- you need isolation
- you need reproducible setup
- the relay is unavailable

## Saved state

Saved state is a fallback, not the first option.

Use it when:

- relay mode is unavailable
- a full dedicated profile is unnecessary
- the task only needs cookies or local storage restored

## Safety

- do not close the browser in relay mode unless the user asked
- verify the attached tab before interacting
- prefer managed mode for risky or isolated flows
