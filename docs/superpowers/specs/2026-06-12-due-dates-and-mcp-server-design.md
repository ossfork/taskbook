# Design: Task Due Dates + MCP Server

Date: 2026-06-12
Status: Approved (autonomous goal session — decisions made by Claude per /goal directive)

## Goal

1. Add the possibility to set a due date on tasks.
2. Add an MCP server that works with the (remote) server storage, usable from Claude Code.

## Part 1: Due Dates

### Data model (taskbook-common)

Add to `Task`:

```rust
#[serde(rename = "_dueDate", default, skip_serializing_if = "Option::is_none")]
pub due_date: Option<i64>,
```

- Epoch **milliseconds** at local midnight of the due date — consistent with the
  existing `_timestamp` convention and cheap to compare for overdue checks.
- `default` + `skip_serializing_if` keeps the JSON format fully backward
  compatible with existing storage files and the original Node.js taskbook.

A new `taskbook_common::due` module owns parsing/formatting:

- `parse_due_date(&str) -> Option<i64>` — accepts `YYYY-MM-DD`, `today`,
  `tomorrow`. Returns local-midnight millis.
- `format_due_date(i64) -> String` — `YYYY-MM-DD` for display/API output.
- `DueStatus` (`Overdue` / `DueToday` / `Upcoming`) + `due_status(due, now)` —
  shared by CLI render and TUI so coloring logic lives in one place.

### CLI input

- **Create**: `tb --task Pay rent due:2026-07-01` (also `due:today`,
  `due:tomorrow`). `board::parse_cli_input` gains a `due:` token; its 4-tuple
  return is replaced by a `ParsedInput` struct (boards, description, priority,
  tags, due_date) since a 5-tuple crosses the readability line.
  An unparseable `due:` value is an error (rendered hint), not silently part of
  the description.
- **Update**: new `--due` flag: `tb --due @3 2026-07-01`, `tb --due @3 none`
  (clears). Mirrors `--priority` structure: `Taskbook::update_due_date(input)`
  plus `set_due_date_silent(id, Option<i64>)` for TUI/MCP.

### Rendering

- Board/timeline item lines get a due suffix for incomplete tasks:
  `due:YYYY-MM-DD`, colored error when overdue, warning when due today,
  muted otherwise. Completed tasks show no due suffix.
- TUI: same suffix logic in the item list widget, using existing theme colors;
  `due:` token also works in the TUI command bar (it reuses
  `parse_cli_input`-based creation paths where applicable).

### Non-goals (YAGNI)

- No times-of-day, no recurrence, no reminders/notifications.
- No due-date sorting mode (existing sort methods untouched).

## Part 2: MCP Server

### Approach decision

Considered:

1. **New crate `taskbook-mcp`** — clean separation but requires converting
   taskbook-client into lib+bin and shipping/packaging another binary.
2. **`tb --mcp` mode inside taskbook-client** (chosen) — zero packaging
   changes, full reuse of config/credentials/storage/encryption, and the MCP
   server automatically talks to the **server storage** when
   `sync.enabled = true` (RemoteStorage with client-side AES-256-GCM), exactly
   like the rest of the client. Falls back to local storage when sync is off.
3. External SDK (`rmcp`, tokio) — pulls an async runtime into a blocking
   binary for a protocol slice (stdio, tools-only) that is small enough to
   implement directly.

### Protocol

`tb --mcp` runs a synchronous stdio MCP server: newline-delimited JSON-RPC 2.0.

- `initialize` → protocol version negotiation (support `2025-06-18`, fall back
  to `2024-11-05`), capabilities `{ "tools": {} }`, serverInfo.
- `notifications/initialized` → ignored (no response to notifications).
- `ping` → `{}`.
- `tools/list` → tool definitions with JSON Schema `inputSchema`.
- `tools/call` → dispatch; results as `content: [{type: "text", text: <json>}]`,
  failures as `isError: true` with the error message (protocol errors only for
  malformed requests/unknown methods/tools).
- stdout carries only JSON-RPC; all diagnostics go to stderr. Only the
  `*_silent` Taskbook methods are used (no render output).

Module layout: `taskbook-client/src/mcp/mod.rs` (server loop + JSON-RPC),
`mcp/tools.rs` (tool schemas + dispatch into `Taskbook`).

### Tools

| Tool | Arguments | Behavior |
|------|-----------|----------|
| `list_items` | `archived?`, `filter?` (pending/in_progress/done/task/note/starred), `board?`, `tag?`, `query?` | Returns items as JSON (id, type, description, boards, tags, priority, due_date, state, starred, date) |
| `list_boards` | — | All board names |
| `create_task` | `description` (req), `boards?`, `priority?`, `tags?`, `due_date?` (YYYY-MM-DD/today/tomorrow) | Returns new id |
| `create_note` | `description` (req), `boards?`, `tags?` | Returns new id |
| `set_task_state` | `id`, `state`: pending/in_progress/done | Deterministic (not a toggle — toggles are hostile to agents) |
| `edit_item` | `id`, plus any of `description`, `priority`, `due_date` (or `"none"`), `boards`, `tags`, `starred` | Partial update |
| `delete_items` | `ids` | Moves to archive |
| `restore_items` | `ids` | Restores from archive |

New `Taskbook` support: `with_storage(Box<dyn StorageBackend>)` constructor
(testability + explicit backend), `set_task_state_silent`, `set_due_date_silent`,
`set_starred_silent`, and a read method returning items without rendering
(already exists: `get_all_items` / `get_all_archive_items`).

### Claude Code usage

```bash
claude mcp add taskbook -- tb --mcp
```

Works against server storage when the user is logged in and `sync.enabled`
is true in `~/.taskbook.json`.

## Error handling

- Invalid due date strings: explicit error at every boundary (CLI render hint,
  MCP `isError` result). Never silently dropped.
- MCP: unknown id → tool error with message; storage/network errors propagate
  as tool errors so the calling agent can react.

## Testing

- `taskbook-common`: due parsing/formatting/status tests; Task serde
  round-trip with and without `_dueDate` (backward compat both directions);
  `parse_cli_input` with `due:` tokens.
- `taskbook-client`: MCP request/response unit tests over an in-memory/local
  temp-dir backend via `Taskbook::with_storage` (initialize, tools/list,
  tools/call happy path + error paths); due-date update command tests.
- Full suite: `cargo fmt --check`, `cargo clippy`, `cargo build`, `cargo test`.

## Docs

Update CLAUDE.md and HELP_TEXT: `due:` syntax, `--due`, `--mcp`, Claude Code
registration snippet.
