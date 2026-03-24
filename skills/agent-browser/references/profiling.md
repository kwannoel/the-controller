# Profiles

Profiles are the managed-mode fallback. They are not the primary path when the user wants to reuse an already-open browser tab.

## When to use a profile

Use a profile when:

- the relay is unavailable
- the user wants isolation from their daily browser
- the task needs a long-lived automation environment

## Basic usage

```bash
# Create or reuse a dedicated profile
agent-browser --profile ~/.profiles/myapp open https://app.example.com/login

# Reuse the same profile later
agent-browser --profile ~/.profiles/myapp open https://app.example.com/dashboard
```

Keep one profile per role or account when isolation matters.

## Practical advice

- use short descriptive profile paths
- do not point at the user's primary personal browser profile
- treat profile directories as sensitive because they may contain live session data
