---
name: setting-credentials
description: Use when an agent needs to set an API key, secret, or credential in a project .env file — ensures secrets never leak into terminal output or conversation context
---

# Setting Credentials

## Overview

Use `controller-cli` to write secrets to `.env` files securely. The secret value is entered through a modal in The Controller UI — it never appears in the terminal, agent output, or conversation context.

## Usage

```bash
controller-cli env set --project <project> --key <ENV_KEY>
```

The CLI connects to The Controller app via Unix socket, opens a secure input modal, and writes the value to the project's `.env` file. Output is redacted — only the key name and status are printed.

## Finding the project name

- Check the `THE_CONTROLLER_SESSION_ID` env var if running inside a Controller session
- Otherwise, use the working directory name (e.g. `my-app` for `/path/to/my-app`)
- The project must already be known to The Controller

## What happens

1. CLI sends request to The Controller app
2. A secure modal opens in the app UI
3. User enters the secret value (masked input)
4. Value is written to the project's `.env` file
5. CLI prints redacted confirmation (e.g. `created OPENAI_API_KEY for my-app`)
