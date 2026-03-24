# Authentication Patterns

Use relay mode first. If the user already has the site open and logged in inside Chrome or Chromium, attach the relay to that tab and continue from there.

## Recommended order

1. Existing logged-in tab in relay mode
2. Managed dedicated browser profile
3. Saved or imported auth state

## Relay mode

Relay mode is the fastest path when the user wants to avoid reauthentication.

Flow:

1. The user opens the target site in Chrome or Chromium.
2. The user attaches the relay extension on the correct tab.
3. You verify the page with `agent-browser snapshot -i`, `agent-browser get url`, or `agent-browser get title`.
4. You continue the task on that live session.

Use relay mode when:

- the user is already authenticated
- the task depends on extensions, cookies, or in-progress state
- the user wants the agent to work in the real browser

Security notes:

- treat the attached tab as sensitive
- avoid unrelated tabs
- detach after finishing
- do not close the browser unless the user asks

Recovery steps:

- wrong tab attached: ask the user to attach the intended tab, then re-run `snapshot -i`
- relay not active: switch to managed mode
- auth expired: let the user refresh the session in the browser, then continue

## Managed dedicated profile

Use a dedicated profile when the user wants isolation or the relay is unavailable.

```bash
# First run: log in once
agent-browser --profile ~/.profiles/myapp open https://app.example.com/login

# Later runs: reuse the same authenticated profile
agent-browser --profile ~/.profiles/myapp open https://app.example.com/dashboard
```

This keeps automation separate from the user's daily browser.

## Imported or saved state

Saved state is an edge-case fallback. It helps when the relay is unavailable and a full dedicated profile is too heavy.

```bash
# Import auth from a compatible running browser session
agent-browser --auto-connect state save ./auth.json

# Reuse it later
agent-browser --state ./auth.json open https://app.example.com/dashboard
```

State files hold session tokens. Keep them out of git and delete them when they are no longer needed.

## Two-factor authentication

Relay mode handles 2FA best because the user can complete the challenge in the real browser tab without moving sessions around.

Fallback flow:

1. attach or open the target login page
2. wait for the user to finish 2FA
3. verify the post-login URL
4. continue on the authenticated page

## Basic verification

After any login or restore step, verify with:

```bash
agent-browser get url
agent-browser get title
agent-browser snapshot -i
```

If the page still looks like a login screen, the session is not ready.
