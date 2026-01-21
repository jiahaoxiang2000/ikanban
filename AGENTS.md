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
TUI/Web Client <--WebSocket--> iKanban Core <--> SQLite
```

All communication uses WebSocket (no REST API). Single endpoint: `GET /api/ws`

## Data Model

```
Project (1:1 Repo)
  └── Tasks (1:N)
        └── Sessions (1:N, each run)
              └── ExecutionProcess (1:N)
```

## WebSocket Protocol

### Connection
- Endpoint: `ws://server/api/ws`
- All operations use request/response pattern over single WebSocket connection
- Events are broadcast to subscribed clients

### Message Types

**Request** (client → server):
```json
{"type": "Request", "payload": {"id": "uuid", "action": "ListProjects"}}
```

**Response** (server → client):
```json
{"type": "Response", "payload": {"id": "uuid", "status": "Success", "data": {...}}}
```

**Event** (server → client, broadcast):
```json
{"type": "Event", "payload": {"event": "ProjectCreated", "payload": {...}}}
```

### Operations

**Projects**: ListProjects, GetProject, CreateProject, UpdateProject, DeleteProject  
**Tasks**: ListTasks, GetTask, CreateTask, UpdateTask, DeleteTask  
**Sessions**: ListSessions, GetSession, CreateSession  
**Executions**: ListExecutions, GetExecution, CreateExecution, StopExecution  
**Logs**: GetExecutionLogs, CreateExecutionLog  
**Subscriptions**: Subscribe, Unsubscribe (Projects | Tasks | ExecutionLogs)

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
- **TUI**: ratatui, crossterm, tokio-tungstenite

## Database & Migrations

Uses **SeaORM** with auto-migrations:

- **Entities**: `crates/ikanban-core/src/entities/` - Define schema as Rust structs
- **Migrations**: `crates/ikanban-core/src/migrator/` - Versioned migrations
- Migrations run automatically on startup via `Migrator::up()`
- To add new tables/columns:
  1. Create new migration file in `migrator/` (e.g., `m20240102_000002_add_sessions.rs`)
  2. Add to `migrator/mod.rs` migrations list
  3. Create/update entity in `entities/`
