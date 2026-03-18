# Deploy Credentials Permissions Design

## Summary

Tighten deploy credential persistence so `~/.the-controller/deploy-credentials.json` is created and kept with owner-only permissions on Unix. This removes reliance on process `umask` for a file that stores long-lived infrastructure API keys.

## Goals

- Create the deploy credentials file with explicit `0600` permissions on Unix.
- Preserve `0600` permissions when the file is updated.
- Keep the existing JSON format and public `DeployCredentials` API unchanged.

## Non-Goals

- Moving secrets into the OS keychain.
- Changing the credential file location or schema.
- Introducing platform-specific behavior outside the Unix permission hardening already required by the issue.

## Approach Options

### Option 1: Keep `std::fs::write` and chmod after write

Pros:

- Minimal code churn.

Cons:

- Leaves a create-time window where the file mode is derived from `umask`.
- Does not satisfy the issue’s requirement to create the file with explicit restrictive permissions.

### Option 2: Use `OpenOptions` and set the Unix create mode at file creation time

Pros:

- Creates the file with explicit `0600` permissions on Unix.
- Works for both initial writes and later updates with a single code path.
- Keeps the implementation local to `deploy/credentials.rs`.

Cons:

- Slightly more code than `std::fs::write`.

Recommendation: Option 2.

## Design

Replace the raw `std::fs::write` call with a small helper that opens the credentials file for write, create, and truncate. On Unix, configure the open call with mode `0o600` so newly created files start restrictive. After writing, continue applying `set_permissions(0o600)` so existing files are corrected if they were created earlier with broader access.

Testing will use a temp `HOME` directory and a Unix-only regression test that:

1. Saves credentials and asserts the file mode is `0600`.
2. Manually broadens the mode to simulate a previously insecure file.
3. Saves again and asserts the mode returns to `0600`.

This test covers both creation-time hardening and update-time preservation.
