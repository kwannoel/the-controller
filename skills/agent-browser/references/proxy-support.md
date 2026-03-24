# Proxy Support

Proxy settings matter most in managed mode. Relay mode uses the network environment of the attached Chrome or Chromium tab.

## Relay mode

In relay mode:

- the attached browser tab keeps using the browser's own proxy, VPN, certificates, and extension stack
- you do not get clean automation isolation
- this is usually what the user wants when they ask to reuse the real browser

## Managed mode

Use managed mode when you need deterministic proxy configuration or network isolation.

Questions to answer before using a proxy:

- does the user want the real browser session, or a controlled automation environment?
- does the task require a specific country, IP range, or corporate proxy?
- would routing through the real browser leak unrelated account state?

If those constraints matter, switch to managed mode and configure the browser there instead of working through relay mode.
