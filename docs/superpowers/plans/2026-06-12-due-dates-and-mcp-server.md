# Task Due Dates + MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional due dates to tasks (create, update, render in CLI + TUI) and an MCP stdio server mode (`tb --mcp`) exposing taskbook operations over the configured storage backend (remote server storage when sync is enabled).

**Architecture:** Due dates are stored as `Option<i64>` epoch-millis (`_dueDate`, omitted when unset → backward compatible JSON). Parsing/formatting/status live in a new `taskbook_common::due` module. The MCP server is a synchronous newline-delimited JSON-RPC 2.0 loop in `taskbook-client/src/mcp/`, reusing `Taskbook` silent methods — no new runtime or SDK dependencies.

**Tech Stack:** Rust, serde/serde_json, chrono, clap, ratatui. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-06-12-due-dates-and-mcp-server-design.md`

---

### Task 1: `taskbook_common::due` module

**Files:**
- Create: `crates/taskbook-common/src/due.rs`
- Modify: `crates/taskbook-common/src/lib.rs` (add `pub mod due;`)

- [ ] **Step 1: Write `due.rs` with tests**

```rust
//! Due-date parsing, formatting and status classification.
//!
//! Due dates are stored as epoch milliseconds at local midnight of the due
//! day, consistent with the `_timestamp` convention used elsewhere.

use chrono::{DateTime, Local, NaiveDate, TimeZone};

/// Classification of a due date relative to today (local time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueStatus {
    Overdue,
    DueToday,
    Upcoming,
}

/// Parse a due-date string into local-midnight epoch millis.
///
/// Accepts `YYYY-MM-DD`, `today`, `tomorrow` (case-insensitive).
pub fn parse_due_date(raw: &str) -> Option<i64> {
    let s = raw.trim().to_lowercase();
    let date = match s.as_str() {
        "today" => Local::now().date_naive(),
        "tomorrow" => Local::now().date_naive().succ_opt()?,
        _ => NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()?,
    };
    date_to_millis(date)
}

fn date_to_millis(date: NaiveDate) -> Option<i64> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|dt| dt.timestamp_millis())
}

/// Format a due-date millis value as `YYYY-MM-DD` in local time.
pub fn format_due_date(millis: i64) -> String {
    DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Classify a due date against the current local date.
pub fn due_status(due_millis: i64) -> DueStatus {
    let due = DateTime::from_timestamp_millis(due_millis)
        .map(|dt| dt.with_timezone(&Local).date_naive());
    let today = Local::now().date_naive();
    match due {
        Some(d) if d < today => DueStatus::Overdue,
        Some(d) if d == today => DueStatus::DueToday,
        _ => DueStatus::Upcoming,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso_date_round_trips() {
        let millis = parse_due_date("2026-07-01").unwrap();
        assert_eq!(format_due_date(millis), "2026-07-01");
    }

    #[test]
    fn parse_today_and_tomorrow() {
        let today = Local::now().date_naive();
        let t = parse_due_date("today").unwrap();
        assert_eq!(format_due_date(t), today.format("%Y-%m-%d").to_string());
        let tm = parse_due_date("Tomorrow").unwrap();
        assert_eq!(
            format_due_date(tm),
            today.succ_opt().unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert_eq!(parse_due_date("next week"), None);
        assert_eq!(parse_due_date("2026-13-99"), None);
        assert_eq!(parse_due_date(""), None);
    }

    #[test]
    fn status_classification() {
        let today = parse_due_date("today").unwrap();
        assert_eq!(due_status(today), DueStatus::DueToday);
        let tomorrow = parse_due_date("tomorrow").unwrap();
        assert_eq!(due_status(tomorrow), DueStatus::Upcoming);
        let yesterday = today - 24 * 60 * 60 * 1000;
        assert_eq!(due_status(yesterday), DueStatus::Overdue);
    }
}
```

- [ ] **Step 2: Export from lib.rs** — add `pub mod due;` alongside existing module declarations.
- [ ] **Step 3: Run** `cargo test -p taskbook-common due` — expect all new tests PASS.
- [ ] **Step 4: Commit** `feat: add due-date parsing module to taskbook-common`

### Task 2: `Task.due_date` field

**Files:**
- Modify: `crates/taskbook-common/src/models/task.rs`

- [ ] **Step 1: Add field after `priority`:**

```rust
    #[serde(rename = "_dueDate", default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<i64>,
```

Initialize `due_date: None` in `Task::new`, and add a builder method:

```rust
    /// Returns the task with the given due date set (epoch millis).
    pub fn with_due_date(mut self, due_date: Option<i64>) -> Self {
        self.due_date = due_date;
        self
    }
```

- [ ] **Step 2: Add serde compat tests** in `task.rs` tests module:

```rust
    #[test]
    fn test_due_date_serde_round_trip() {
        let task = Task::new(1, "T".to_string(), vec!["My Board".to_string()], 1)
            .with_due_date(Some(1_790_000_000_000));
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("_dueDate"));
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back.due_date, Some(1_790_000_000_000));
    }

    #[test]
    fn test_task_without_due_date_omits_field_and_deserializes() {
        let task = Task::new(1, "T".to_string(), vec!["My Board".to_string()], 1);
        let json = serde_json::to_string(&task).unwrap();
        assert!(!json.contains("_dueDate"));
        // Legacy JSON without the field still parses
        let legacy = r#"{"_id":1,"_date":"Fri Jun 12 2026","_timestamp":1,"_isTask":true,"description":"x","isStarred":false,"isComplete":false,"inProgress":false,"priority":1,"boards":["My Board"]}"#;
        let back: Task = serde_json::from_str(legacy).unwrap();
        assert_eq!(back.due_date, None);
    }
```

(`serde_json` is already a dependency of taskbook-common.)

- [ ] **Step 3: Run** `cargo test -p taskbook-common` — PASS.
- [ ] **Step 4: Commit** `feat: add optional due_date field to Task`

### Task 3: `parse_cli_input` → `ParsedInput` with `due:` token

**Files:**
- Modify: `crates/taskbook-common/src/board.rs`
- Modify: `crates/taskbook-client/src/taskbook.rs` (`get_options`, `CreateOptions`, `create_task`)

- [ ] **Step 1: Replace the tuple return with a struct + error enum** in `board.rs`:

```rust
/// Structured result of parsing CLI create input.
#[derive(Debug)]
pub struct ParsedInput {
    pub boards: Vec<String>,
    pub description: String,
    pub priority: u8,
    pub tags: Vec<String>,
    pub due_date: Option<i64>,
}

/// Errors from parsing CLI create input.
#[derive(Debug, PartialEq, Eq)]
pub enum CliParseError {
    /// The value after `due:` was not a valid date.
    InvalidDueDate(String),
}
```

`parse_cli_input` keeps its loop, plus a `due:` arm before the `@`/`+` checks:

```rust
        } else if let Some(value) = word.strip_prefix("due:") {
            match crate::due::parse_due_date(value) {
                Some(millis) => due_date = Some(millis),
                None => return Err(CliParseError::InvalidDueDate(value.to_string())),
            }
        }
```

Signature: `pub fn parse_cli_input(input: &[String]) -> Result<ParsedInput, CliParseError>`.

- [ ] **Step 2: Update existing board.rs tests** to destructure the struct (e.g. `let parsed = parse_cli_input(&input).unwrap();` then assert on `parsed.boards` etc.) and add:

```rust
    #[test]
    fn test_parse_cli_input_with_due_date() {
        let input: Vec<String> = vec!["Pay".into(), "rent".into(), "due:2026-07-01".into()];
        let parsed = parse_cli_input(&input).unwrap();
        assert_eq!(parsed.description, "Pay rent");
        assert_eq!(
            parsed.due_date,
            Some(crate::due::parse_due_date("2026-07-01").unwrap())
        );
    }

    #[test]
    fn test_parse_cli_input_invalid_due_date_errors() {
        let input: Vec<String> = vec!["task".into(), "due:soon".into()];
        assert_eq!(
            parse_cli_input(&input).unwrap_err(),
            CliParseError::InvalidDueDate("soon".to_string())
        );
    }
```

- [ ] **Step 3: Update the only caller**, `Taskbook::get_options` in `taskbook.rs`: add `due_date: Option<i64>` to `CreateOptions`, map `CliParseError::InvalidDueDate` to `self.render.invalid_due_date()` + `Err(TaskbookError::General(...))` (render method added in Task 4 — do Tasks 3+4 together if needed to keep the build green; otherwise temporarily render via the existing `missing_desc` is NOT acceptable — add `invalid_due_date` now as part of this task). In `create_task`, build the task with `.with_due_date(options.due_date)`.
- [ ] **Step 4: Run** `cargo test -p taskbook-common && cargo build` — PASS.
- [ ] **Step 5: Commit** `feat: parse due: token in CLI create input`

### Task 4: `--due` flag end-to-end + render messages

**Files:**
- Modify: `crates/taskbook-client/src/render.rs`, `src/taskbook.rs`, `src/commands.rs`, `src/main.rs`

- [ ] **Step 1: render.rs messages** (next to `success_priority` / `invalid_priority`):

```rust
    pub fn invalid_due_date(&self) {
        eprintln!(
            "\n {} Invalid due date. Use YYYY-MM-DD, today, tomorrow or none",
            self.error("✖")
        );
    }

    pub fn success_due_date(&self, id: u64, due_date: Option<i64>) {
        match due_date {
            Some(millis) => println!(
                "\n {} Set due date of task: {} to {}",
                self.success("✔"),
                self.muted(&id.to_string()),
                self.muted(&due::format_due_date(millis))
            ),
            None => println!(
                "\n {} Cleared due date of task: {}",
                self.success("✔"),
                self.muted(&id.to_string())
            ),
        }
    }
```

(import `taskbook_common::due`.)

- [ ] **Step 2: taskbook.rs** — mirror `update_priority`:

```rust
    pub fn update_due_date(&self, input: &[String]) -> Result<()> {
        let targets: Vec<&String> = input.iter().filter(|x| x.starts_with('@')).collect();

        if targets.is_empty() {
            self.render.missing_id();
            return Err(TaskbookError::InvalidId(0));
        }
        if targets.len() > 1 {
            self.render.invalid_ids_number();
            return Err(TaskbookError::InvalidId(0));
        }

        let target = targets[0];
        let id: u64 = target
            .trim_start_matches('@')
            .parse()
            .map_err(|_| TaskbookError::InvalidId(0))?;

        let value = input.iter().find(|x| *x != target);
        let due_date = match value.map(|s| s.as_str()) {
            Some("none") => None,
            Some(raw) => match due::parse_due_date(raw) {
                Some(millis) => Some(millis),
                None => {
                    self.render.invalid_due_date();
                    return Err(TaskbookError::General(format!("invalid due date: {raw}")));
                }
            },
            None => {
                self.render.invalid_due_date();
                return Err(TaskbookError::General("missing due date".to_string()));
            }
        };

        self.set_due_date_silent(id, due_date)?;
        self.render.success_due_date(id, due_date);
        Ok(())
    }

    /// Set or clear a task due date without CLI output (for TUI/MCP)
    pub fn set_due_date_silent(&self, id: u64, due_date: Option<i64>) -> Result<()> {
        let mut data = self.get_data()?;
        let existing_ids = self.get_ids(&data);
        self.validate_ids_silent(&[id], &existing_ids)?;

        if let Some(item) = data.get_mut(&id.to_string()) {
            let task = item
                .as_task_mut()
                .ok_or_else(|| TaskbookError::General(format!("item {id} is not a task")))?;
            task.due_date = due_date;
        }

        self.save(&data)
    }
```

(import `taskbook_common::due`.)

- [ ] **Step 3: commands.rs + main.rs** — add `due: bool` to `commands::run` (route `taskbook.update_due_date(&input)`), add `#[arg(long)] due: bool` to `Cli`, include in `has_action_flags`, pass through. Update `HELP_TEXT`: `--due` line + examples `$ tb --task Pay rent due:2026-07-01`, `$ tb --due @3 2026-07-01`, `$ tb --due @3 none`.
- [ ] **Step 4: Run** `cargo build && cargo test` — PASS.
- [ ] **Step 5: Commit** `feat: add --due flag to set/clear task due dates`

### Task 5: CLI rendering of due dates

**Files:**
- Modify: `crates/taskbook-client/src/render.rs`

- [ ] **Step 1: Add suffix helper** (next to `get_star`):

```rust
    fn get_due(&self, item: &StorageItem) -> String {
        let Some(task) = item.as_task() else {
            return String::new();
        };
        let (Some(millis), false) = (task.due_date, task.is_complete) else {
            return String::new();
        };
        let text = format!("due:{}", due::format_due_date(millis));
        match due::due_status(millis) {
            due::DueStatus::Overdue => self.error(&text).to_string(),
            due::DueStatus::DueToday => self.warning(&text).to_string(),
            due::DueStatus::Upcoming => self.muted(&text).to_string(),
        }
    }
```

- [ ] **Step 2: Wire into `display_item_by_board` and `display_item_by_date`**: compute `let due = self.get_due(item);` and push into `suffix_parts` after tags, before age/boards.
- [ ] **Step 3: Manual check**: `cargo run -p taskbook-client -- --cli --taskbook-dir /tmp/tb-due-test --task Demo due:tomorrow` then `tb --cli --taskbook-dir /tmp/tb-due-test` shows `due:<date>` suffix.
- [ ] **Step 4: Commit** `feat: render due dates in board and timeline views`

### Task 6: TUI support (create token + row display)

**Files:**
- Modify: `crates/taskbook-client/src/tui/command_parser.rs`, `src/tui/actions.rs`, `src/tui/widgets/item_row.rs`, `src/taskbook.rs`

- [ ] **Step 1: `ParsedCommand::Task`** gains `due_date: Option<i64>`. In `parse_task`, add a token arm:

```rust
        } else if let Some(value) = token.strip_prefix("due:") {
            match due::parse_due_date(value) {
                Some(millis) => due_date = Some(millis),
                None => {
                    return Err(ParseError {
                        message: format!("Invalid due date: {value} (use YYYY-MM-DD, today, tomorrow)"),
                    })
                }
            }
        }
```

Update the usage string to `"Usage: /task [@board] description [p:1-3] [due:YYYY-MM-DD]"` and existing tests' destructuring. Add a test:

```rust
    #[test]
    fn test_parse_task_with_due_date() {
        let cmd = parse_command("/task Pay rent due:2026-07-01").unwrap();
        match cmd {
            ParsedCommand::Task { due_date, description, .. } => {
                assert_eq!(description, "Pay rent");
                assert_eq!(due_date, due::parse_due_date("2026-07-01"));
            }
            _ => panic!("expected Task"),
        }
    }
```

- [ ] **Step 2: `create_task_direct_with_tags`** gains `due_date: Option<i64>` param (build task with `.with_due_date(due_date)`); `create_task_direct` passes `None`; `actions.rs` destructures and forwards `due_date`.
- [ ] **Step 3: `item_row.rs`** — after the priority indicator block:

```rust
    // Due date
    if let Some(task) = item.as_task() {
        if !task.is_complete {
            if let Some(millis) = task.due_date {
                let style = match due::due_status(millis) {
                    due::DueStatus::Overdue => app.theme.error,
                    due::DueStatus::DueToday => app.theme.warning,
                    due::DueStatus::Upcoming => app.theme.muted,
                };
                spans.push(Span::styled(
                    format!(" due:{}", due::format_due_date(millis)),
                    style,
                ));
            }
        }
    }
```

- [ ] **Step 4: Run** `cargo test -p taskbook-client && cargo build` — PASS.
- [ ] **Step 5: Commit** `feat: due-date support in TUI (due: token, row display)`

### Task 7: Taskbook support methods for MCP

**Files:**
- Modify: `crates/taskbook-client/src/taskbook.rs`

- [ ] **Step 1: Add:**

```rust
    /// Construct a Taskbook over an explicit storage backend (for MCP/tests).
    pub fn with_storage(storage: Box<dyn StorageBackend>) -> Self {
        Self {
            storage,
            render: Render::new(Config::default()),
        }
    }

    /// Set a task's state deterministically without CLI output (for MCP)
    pub fn set_task_state_silent(&self, id: u64, state: &str) -> Result<()> {
        let mut data = self.get_data()?;
        let existing_ids = self.get_ids(&data);
        self.validate_ids_silent(&[id], &existing_ids)?;

        if let Some(item) = data.get_mut(&id.to_string()) {
            let task = item
                .as_task_mut()
                .ok_or_else(|| TaskbookError::General(format!("item {id} is not a task")))?;
            match state {
                "done" => {
                    task.is_complete = true;
                    task.in_progress = false;
                }
                "in_progress" => {
                    task.is_complete = false;
                    task.in_progress = true;
                }
                "pending" => {
                    task.is_complete = false;
                    task.in_progress = false;
                }
                other => {
                    return Err(TaskbookError::General(format!(
                        "invalid state '{other}' (use pending, in_progress or done)"
                    )))
                }
            }
        }

        self.save(&data)
    }

    /// Set starred flag explicitly without CLI output (for MCP)
    pub fn set_starred_silent(&self, id: u64, starred: bool) -> Result<()> {
        let mut data = self.get_data()?;
        let existing_ids = self.get_ids(&data);
        self.validate_ids_silent(&[id], &existing_ids)?;

        if let Some(item) = data.get_mut(&id.to_string()) {
            item.set_starred(starred);
        }

        self.save(&data)
    }
```

- [ ] **Step 2: Run** `cargo build` — PASS (methods exercised by Task 8 tests).
- [ ] **Step 3: Commit** `feat: explicit-state Taskbook methods and storage injection`

### Task 8: MCP server (`tb --mcp`)

**Files:**
- Create: `crates/taskbook-client/src/mcp/mod.rs` (JSON-RPC loop + tests)
- Create: `crates/taskbook-client/src/mcp/tools.rs` (tool schemas + dispatch)
- Modify: `crates/taskbook-client/src/main.rs` (flag + routing, before TUI/CLI decision)

Protocol: newline-delimited JSON-RPC 2.0 on stdio. `initialize` negotiates version (`2025-06-18`, `2025-03-26`, `2024-11-05`), capabilities `{"tools":{}}`. Notifications (no `id`) → no response. Unknown method → `-32601`; unknown tool / malformed args → `-32602`; tool runtime failures → `isError: true` result. Tools (all results `content:[{type:"text",text:<json>}]`): `list_items`, `list_boards`, `create_task`, `create_note`, `set_task_state`, `edit_item`, `delete_items`, `restore_items` (full schemas and dispatch in tools.rs as specced; `serve()` is generic over `BufRead`/`Write` for testability).

- [ ] **Step 1: Write mod.rs with serve loop and tests** covering: initialize handshake, tools/list returns 8 tools, create_task → list_items round trip over a temp-dir LocalStorage via `Taskbook::with_storage`, set_task_state invalid state → isError, unknown method → -32601, notification gets no response.
- [ ] **Step 2: Write tools.rs**: `definitions() -> Value` (array of tool objects with JSON Schema), `call(taskbook, params) -> Result<Value, (i64, String)>` dispatching to Taskbook methods; `item_to_json` includes id, type, description, boards, tags, starred, created, and for tasks state/priority/due_date (formatted `YYYY-MM-DD`).
- [ ] **Step 3: main.rs**: `#[arg(long)] mcp: bool`; before the TUI/CLI decision: `if cli.mcp { ... mcp::run(cli.taskbook_dir.as_deref()) ... return; }`; `mod mcp;`. HELP_TEXT: `--mcp  Run as MCP stdio server (for Claude Code etc.)`.
- [ ] **Step 4: Run** `cargo test -p taskbook-client mcp` — PASS. Manual smoke: `printf '...initialize...\n...tools/list...\n' | cargo run -p taskbook-client -- --mcp --taskbook-dir /tmp/tb-mcp-test`.
- [ ] **Step 5: Commit** `feat: add MCP stdio server mode (tb --mcp)`

### Task 9: Docs + full verification

**Files:**
- Modify: `CLAUDE.md` (CLI usage: due syntax, `--due`, `--mcp`; architecture tree: `mcp/`; MCP section with `claude mcp add taskbook -- tb --mcp`)

- [ ] **Step 1: Update CLAUDE.md.**
- [ ] **Step 2: Run full suite**: `cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo build --workspace && cargo test --workspace` — all PASS.
- [ ] **Step 3: Commit** `docs: document due dates and MCP server`
