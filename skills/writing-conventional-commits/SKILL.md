---
name: writing-conventional-commits
description: Use when writing or amending git commit messages in this repo so the summary line follows Conventional Commits 1.0.0 with a valid type and concise description
---

# Writing Conventional Commits

Write the summary line as:

```text
<type>: <description>
```

Use lowercase types. Prefer:

- `feat` for user-facing features
- `fix` for bug fixes
- `docs` for documentation-only changes
- `refactor` for behavior-preserving code restructuring
- `test` for test-only changes
- `chore` for tooling, dependency, config, or maintenance work

Keep the description concise, imperative, and specific to the diff. Do not end the summary with a period.

For mixed changes, choose the type that best describes the primary reason the commit exists. If the commit mostly changes package management, build tooling, or repo workflow, use `chore`.

Before committing or amending:

1. Inspect the staged diff.
2. Choose the narrowest valid type.
3. Write a summary that states the change, not the implementation details.
4. Verify `git log -1 --oneline` matches the Conventional Commits summary format.
