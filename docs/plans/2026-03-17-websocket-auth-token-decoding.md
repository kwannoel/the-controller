# WebSocket Auth Token Decoding Plan

1. Add a failing Rust integration test for authenticated API and percent-encoded WebSocket token auth with reserved characters.
2. Update server auth middleware to percent-decode the `token` query parameter before comparing it to `CONTROLLER_AUTH_TOKEN`.
3. Re-run the targeted test for a red-green check, then run the required repo verification commands.
4. Self-review the diff, commit with a conventional message including `closes #15` and the required trailer, then create and merge a PR.
