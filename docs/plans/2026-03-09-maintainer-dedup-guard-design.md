# Deterministic Maintainer Dedup Guard Design

## Definition

Issue #292 reports that the maintainer pipeline creates semantically duplicate `filed-by-maintainer` issues because dedup is left to model judgment in prompt text. We need a deterministic guard that routes duplicate findings to update/comment on the existing issue and only files new issues when no sufficiently similar issue exists.

## Constraints

- Keep current maintainer scheduler and `MaintainerRunLog` data model behavior intact.
- Enforce dedup in Rust code (deterministic/testable), not solely in LLM prompt instructions.
- Reuse existing `gh` CLI workflow (`issue list`, `issue create`, `issue edit`, `issue comment`).
- Match semantic duplicates using normalized fingerprint data from finding metadata (`affected_files`, `symptom_type`, `keywords`) with deterministic thresholding.
- Provide unit tests in `src-tauri/src/maintainer.rs` using mocked existing issues and candidate findings.

## Approaches Considered

1. Prompt-only dedup (reject): still nondeterministic; same failure mode as current pipeline.
2. Hybrid model-proposed dedup + Rust fallback (reject): improved, but still relies on model routing.
3. Rust-owned dedup and issue actions (selected): deterministic and fully unit-testable.

## Selected Architecture

1. Codex returns findings JSON only (no direct `gh issue create/edit/comment`).
2. Rust fetches open `filed-by-maintainer` issues.
3. Rust computes candidate fingerprints from:
   - normalized `affected_files`
   - normalized `symptom_type`
   - normalized `keywords`
4. Rust computes existing issue comparison tokens from:
   - explicit fingerprint metadata in issue body (if present)
   - fallback normalized tokens from existing issue title/body
5. Similarity score = deterministic token overlap ratio against candidate fingerprint.
6. If best score >= threshold, Rust updates/comments matched issue; otherwise creates new issue.
7. New/updated issue bodies include fingerprint metadata for stronger future matching.

## Validation Strategy

- TDD unit tests first for:
  - fingerprint normalization stability
  - duplicate detection against mocked existing issues
  - non-duplicate path below threshold
  - deterministic tie-breaking behavior
- Parsing tests for findings JSON shape.
- Integration-level unit tests for run-log summary assembly from deterministic routing logic.
- `cargo test` focused on `maintainer` module, then full `cargo test` for regression safety.
