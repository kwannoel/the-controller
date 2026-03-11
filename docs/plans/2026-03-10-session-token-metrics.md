# Session Token Metrics — Implementation Plan

## Steps

### 1. Backend: Token data model
- Add `TokenDataPoint` struct to `src-tauri/src/models.rs`
- Fields: timestamp, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens

### 2. Backend: JSONL parser for Claude Code sessions
- New module `src-tauri/src/token_usage.rs`
- Function to derive Claude project directory from worktree path
- Function to find the most recent JSONL file in that directory
- Function to parse Claude JSONL entries with `type: "assistant"` and extract `message.usage`

### 3. Backend: JSONL parser for Codex sessions
- In same module, function to scan `~/.codex/sessions/` directories
- Match files by reading `session_meta.payload.cwd` to find the right session
- Parse `event_msg` entries with `payload.type: "token_count"` — use `last_token_usage` (per-turn) not `total_token_usage`

### 4. Backend: Tauri command
- Add `get_session_token_usage` command to `src-tauri/src/commands.rs`
- Look up session config, delegate to token_usage module based on session kind
- Register in command handler

### 5. Frontend: TokenChart component
- New `src/lib/TokenChart.svelte`
- Raw SVG stacked bar chart
- Props: `dataPoints: TokenDataPoint[]`
- Catppuccin Mocha colors
- Show total tokens as header text

### 6. Frontend: Integrate into SummaryPane
- Import TokenChart into SummaryPane
- Call `get_session_token_usage` command
- Refresh on `session-status-hook` events (same pattern as commit refresh)
- Show chart below DONE section

### 7. Frontend: Backend binding
- Add `getSessionTokenUsage` to `src/lib/backend.ts`

### 8. Validation
- Run `cargo test` for backend
- Run `npx vitest run` for frontend
- Manual verification with a real session
