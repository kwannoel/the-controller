# Session Token Metrics Dashboard

## Problem

No visibility into token consumption per session. Users want to see how many tokens each Claude Code or Codex session is burning.

## Design

### Data Source

Read JSONL session files from disk. Both tools write structured token data:

- **Claude Code**: `~/.claude/projects/<encoded-worktree-path>/<uuid>.jsonl` — entries with `type: "assistant"` have `message.usage.{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}`
- **Codex**: `~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<datetime>-<uuid>.jsonl` — entries with `type: "event_msg"` + `payload.type: "token_count"` have `total_token_usage.{input_tokens, output_tokens, cached_input_tokens, reasoning_output_tokens}`

### Session-to-File Mapping

- **Claude**: Derive project directory from session's `worktree_path` — path separators and dots become `-` (e.g., `/Users/noel/.the-controller/worktrees/foo/session-1` → `~/.claude/projects/-Users-noel--the-controller-worktrees-foo-session-1/`). Read the most recently modified JSONL in that directory.
- **Codex**: Scan `~/.codex/sessions/` for files whose `session_meta.payload.cwd` matches the session's working directory.

### Backend

New Tauri command: `get_session_token_usage(session_id: String, project_id: String) -> Vec<TokenDataPoint>`

```rust
struct TokenDataPoint {
    timestamp: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
}
```

Steps:
1. Look up session config to get `worktree_path` and `kind`
2. Find the right JSONL file(s) based on kind
3. Parse entries, extract per-turn token data
4. Return sorted by timestamp

### Frontend

- New `TokenChart.svelte` — raw SVG stacked bar chart (no chart library dependency)
- Each bar = one assistant turn, stacked by token type
- Placed in `SummaryPane.svelte` below existing PROMPT/DONE sections
- Refreshes on session idle transition (reuse existing `session-status-hook` events)
- Catppuccin Mocha colors: blue for input, green for output, surface for cache

### Non-Goals

- Cost calculation (would need pricing tables)
- Cross-session aggregation (per-session only)
- Real-time streaming (poll on status change is sufficient)
