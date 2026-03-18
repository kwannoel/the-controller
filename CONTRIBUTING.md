# Contributing

Thanks for your interest in The Controller. Here's what to know before opening a PR.

## State your intention

Every PR must explain **why** it exists — not just what it changes. Include the problem you're solving or the improvement you're after.

Good examples:

- *"Sessions don't restore scroll position after switching — this causes me to lose context when jumping between terminals."*
- *"The sidebar flickers on resize because the layout recalculates on every pixel. I debounced it."*
- *"I wanted per-project keybindings so I can use different shortcuts for different workflows."*

Bad examples:

- *"Fixed stuff"*
- *"Refactored sidebar"*
- *"Updated code"*

## Quality bar

- PRs must pass all existing tests (`pnpm test` and `cd src-tauri && cargo test`).
- PRs must also pass the repo's format/lint gates: `pnpm check`, `cd src-tauri && cargo fmt --check`, and `cd src-tauri && cargo clippy -- -D warnings`.
- If your change is behavioral, include a test or explain why one isn't feasible.
- Keep changes focused — one concern per PR.

## Project direction

The Controller is a strongly opinionated power tool built for efficiency, simplicity, and elegant design. It is maintained under a benevolent-dictator model — decisions about direction, scope, and taste ultimately rest with one person.

This is still a period of experimentation — the roadmap itself is unstable. Large feature requests that don't align with the current direction may not be prioritized or accepted. If you're unsure whether something fits, open an issue first to discuss before investing time in a PR.
