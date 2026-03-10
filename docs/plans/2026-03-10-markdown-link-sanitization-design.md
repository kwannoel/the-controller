# Markdown Link Sanitization Design

## Definition

Sanitize markdown link URLs before notes preview HTML is rendered so unsafe schemes such as `javascript:` and `data:` are never emitted into anchor `href` attributes.

## Constraints

- Keep the change scoped to note-preview markdown rendering.
- Preserve existing markdown behavior for allowed links and inline formatting.
- Treat this as a behavior change and implement it with TDD.
- Avoid introducing a broader markdown library or unrelated renderer refactors.

## Approaches

### 1. Allowlist schemes in the existing link replacement

Parse link destinations during inline formatting, allow only `http:`, `https:`, and `mailto:`, and render disallowed destinations as plain escaped text.

Pros: Minimal surface area, direct fix, easy to test.
Cons: Requires slightly more careful string handling than the current regex replacement.

### 2. Post-process rendered HTML anchors

Render markdown as today, then scan the HTML for anchor tags and rewrite or remove unsafe `href` values.

Pros: Leaves inline parsing untouched.
Cons: More brittle, mixes HTML sanitization into renderer output, still requires safe attribute handling.

### 3. Replace the handwritten renderer

Adopt a markdown library with sanitization support.

Pros: Stronger long-term markdown support.
Cons: Too large for this issue and unnecessary for the current bug.

## Chosen Design

Use approach 1. Add a small URL sanitization helper in `src/lib/markdown.ts` that accepts only `http:`, `https:`, and `mailto:` absolute URLs. When a markdown link uses any other scheme or an invalid destination, render just the escaped link text instead of an anchor. Escape the `href` attribute separately so allowed URLs cannot inject attribute-breaking characters.

## Validation

- Add tests proving allowed `https:` and `mailto:` links still render.
- Add tests proving `javascript:` and `data:` destinations do not produce anchors or unsafe `href` output.
- Run the focused markdown test in a failing state first, then rerun after implementation.
- Run the full frontend test suite before opening a PR.
