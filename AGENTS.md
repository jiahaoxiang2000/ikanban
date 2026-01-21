# AGENTS.md

## Rules

- Don't manually change code format. Use formatting tools.
- NOT to update/create markdown docs unless asked.
- Keep output concise.
- When a TODO.md item is completed, update the TODO.md to mark it as done.
- if change the code, need use cargo check to ensure no errors.

## Reference

- Reference impl: `/home/isomo/code/tools/vibe-kanban/crates/` (if conflict, use our code)

## Architecture

```
TUI/Web Client <--HTTP/WS--> iKanban Core <--> SQLite
```

## Data Model

```
Project (1:1 Repo)
  └── Tasks (1:N)
        └── Sessions (1:N, each run)
              └── ExecutionProcess (1:N)
```

## API Endpoints

### Projects

- `GET/POST /api/projects`, `GET/PUT/DELETE /api/projects/{id}`
- `GET /api/projects/stream/ws`

### Tasks

- `GET /api/tasks?project_id={id}`, `POST /api/tasks`
- `GET/PUT/DELETE /api/tasks/{id}`
- `GET /api/tasks/stream/ws?project_id={id}`

### Sessions (Planned)

- `GET/POST /api/tasks/{tid}/sessions`
- `GET /api/tasks/{tid}/sessions/{sid}`

### Executions (Planned)

- `GET/POST .../sessions/{sid}/executions`
- `GET .../executions/{eid}`, `POST .../executions/{eid}/stop`
- `GET .../executions/{eid}/logs`, `GET .../executions/{eid}/logs/stream`

### Merges (Planned)

- `GET/POST /api/projects/{pid}/merges/direct|pr`

### Events

- `GET /api/events` (SSE)

## Data Models

### Project

```rust
Project { id, name, repo_path, archived, pinned, created_at, updated_at }
ProjectWithStatus { project, is_running, is_errored, task_count, active_task_count }
```

### Task

```rust
TaskStatus { Todo, InProgress, InReview, Done, Cancelled }
Task { id, project_id, title, description, status, branch, working_dir, parent_task_id, created_at, updated_at }
TaskWithSessionStatus { task, session_count, has_running_session, last_session_failed }
```

### Session

```rust
SessionStatus { Running, Completed, Failed, Cancelled }
Session { id, task_id, executor, status, started_at, completed_at, created_at, updated_at }
```

### ExecutionProcess

```rust
ExecutionProcessStatus { Running, Completed, Failed, Killed }
ExecutionProcessRunReason { SetupScript, CleanupScript, CodingAgent, DevServer }
ExecutionProcess { id, session_id, run_reason, executor_action, status, exit_code, dropped, started_at, completed_at, ... }
```

### CodingAgentTurn

```rust
CodingAgentTurn { id, execution_process_id, agent_session_id, prompt, summary, seen, created_at, updated_at }
```

### Merge

```rust
MergeStatus { Open, Merged, Closed, Unknown }
DirectMerge { id, project_id, merge_commit, target_branch, created_at }
PrMerge { id, project_id, target_branch, pr_number, pr_url, status, merged_at, created_at }
```

## Tech Stack

- **Core**: axum, tokio, sea-orm (SQLite), serde, uuid, chrono, tracing
- **TUI**: ratatui, crossterm, reqwest, tokio-tungstenite

## Database & Migrations

Uses **SeaORM** with auto-migrations:

- **Entities**: `crates/ikanban-core/src/entities/` - Define schema as Rust structs
- **Migrations**: `crates/ikanban-core/src/migrator/` - Versioned migrations
- Migrations run automatically on startup via `Migrator::up()`
- To add new tables/columns:
  1. Create new migration file in `migrator/` (e.g., `m20240102_000002_add_sessions.rs`)
  2. Add to `migrator/mod.rs` migrations list
  3. Create/update entity in `entities/`
