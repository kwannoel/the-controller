# Markdown Link Sanitization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Prevent unsafe markdown link URLs from reaching notes preview HTML while preserving normal note links.

**Architecture:** Keep the fix inside the handwritten markdown renderer. Add a small helper that allowlists safe absolute URL schemes and use it from link rendering so unsafe destinations fall back to plain text.

**Tech Stack:** Svelte 5, TypeScript, Vitest

---

### Task 1: Add failing coverage for unsafe link destinations

**Files:**
- Modify: `src/lib/markdown.test.ts`
- Test: `src/lib/markdown.test.ts`

**Step 1: Write the failing test**

Add tests asserting:
- `mailto:` links still render as anchors.
- `javascript:` links do not render `<a href=...>`.
- `data:` links do not render `<a href=...>`.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/markdown.test.ts`
Expected: FAIL because unsafe links still render anchors.

### Task 2: Implement minimal URL sanitization

**Files:**
- Modify: `src/lib/markdown.ts`
- Test: `src/lib/markdown.test.ts`

**Step 1: Write minimal implementation**

Add:
- A helper to escape attribute values used in `href`.
- A helper that parses absolute URLs and allowlists `http:`, `https:`, and `mailto:`.
- Link rendering via regex callback so each URL is sanitized before anchor HTML is emitted.

**Step 2: Run targeted tests to verify they pass**

Run: `npx vitest run src/lib/markdown.test.ts`
Expected: PASS

### Task 3: Regression verification and review

**Files:**
- Modify: `docs/plans/2026-03-10-markdown-link-sanitization-design.md`
- Modify: `docs/plans/2026-03-10-markdown-link-sanitization.md`

**Step 1: Run broader verification**

Run: `npx vitest run`
Expected: PASS

**Step 2: Review diff for scope and correctness**

Check that:
- Unsafe schemes never appear in rendered anchors.
- Allowed links still preserve existing target/rel attributes.
- No unrelated note editor behavior changed.

**Step 3: Commit**

Commit with a message that includes `closes #299` and the `Contributed-by: auto-worker` trailer.
