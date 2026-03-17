# Autonomous Execution Skill Design

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-writing-plans to create the implementation plan from this design.

**Goal:** A skill that executes implementation plans fully autonomously — same quality gates as subagent-driven-development, but zero human interaction unless genuinely blocked.

**Architecture:** Wraps subagent-driven-development's per-task cycle (implementer → spec review → code quality review) with an autonomous orchestrator that handles worktree setup, answers subagent questions from plan context, and reports only when all tasks are done.

## What Changes vs. `subagent-driven-development`

Only two things:

1. **Orchestrator answers subagent questions from plan context** instead of surfacing to human. Only surfaces if it truly cannot determine the answer.
2. **No `finishing-a-development-branch`** at the end. Human decides when to merge.

Everything else is identical: fresh subagent per task, spec reviewer, code quality reviewer, final code reviewer, review fix loops.

## Flow

```
1. Set up worktree (using-git-worktrees)
2. Read plan, extract all tasks with full text
3. For each task (sequential):
   a. Dispatch implementer subagent → implement → test → self-review → commit
   b. If implementer asks questions → orchestrator answers from plan context
   c. If orchestrator can't answer → surface to human (only blocker case)
   d. Dispatch spec reviewer → if issues → implementer fixes → re-review loop
   e. Dispatch code quality reviewer → if issues → implementer fixes → re-review loop
   f. Mark task complete
4. Dispatch final code reviewer for entire implementation
5. Report final summary (what was done, files changed, test results)
```

## Implementer Prompt

Reuse `implementer-prompt.md` from `subagent-driven-development` as-is. No changes needed.

## Reviewer Prompts

Reuse `spec-reviewer-prompt.md` and `code-quality-reviewer-prompt.md` from `subagent-driven-development` as-is. No changes needed.

## Blocker Handling

The orchestrator is the first line of defense for subagent questions. It has the full plan text and project context, and can resolve most ambiguities. Only if the orchestrator genuinely cannot determine the answer does it surface to the human.

## Integration

- **Requires:** `the-controller-using-git-worktrees` (workspace setup)
- **Requires:** `the-controller-writing-plans` (creates the plan this skill executes)
- **Reuses prompts from:** `the-controller-subagent-driven-development` (implementer, spec reviewer, code quality reviewer)

## What's NOT Included

- No `finishing-a-development-branch` — human reviews and decides when to merge
- No human checkpoints between tasks
- No "ready for feedback" pauses
