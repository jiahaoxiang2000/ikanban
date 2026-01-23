# iKanban Implementation TODO

## Phase 1: Foundation

| Task                                                        | Status | Deps |
| ----------------------------------------------------------- | ------ | ---- |
| 1.1 Setup Cargo.toml with dependencies                      | [ ]    | -    |
| 1.2 Create db/models.rs (Project, Task, Session, Execution) | [ ]    | 1.1  |
| 1.3 Create db/mod.rs (SQLite connection, migrations)        | [ ]    | 1.2  |

### 1.2 Database Models (MVP - Simplified)

```rust
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
}

pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,  // Todo, InProgress, InReview, Done
    pub created_at: DateTime<Utc>,
}

// MVP: Unified Session (worktree + execution)
pub struct Session {
    pub id: String,
    pub task_id: String,

    // Worktree lifecycle
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,

    // Execution info
    pub executor_type: String,  // "opencode"
    pub status: SessionStatus,  // Running, Completed, Failed, Killed
    pub exit_code: Option<i32>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

pub struct LogEntry {
    pub id: String,
    pub session_id: String,  // Direct link (no execution table)
    pub timestamp: DateTime<Utc>,
    pub log_type: LogType,  // Stdout, Stderr, Event
    pub content: String,
}
```

## Phase 2: Core Managers (Parallel)

> Tasks 2.1, 2.2, 2.3 can run in **parallel** after Phase 1

| Task                                           | Status | Deps | Parallel Group |
| ---------------------------------------------- | ------ | ---- | -------------- |
| 2.1 worktree/manager.rs - WorktreeManager      | [ ]    | 1.3  | A              |
| 2.2 session/manager.rs - SessionManager (base) | [ ]    | 1.3  | A              |
| 2.3 executor/mod.rs - Executor trait + types   | [ ]    | 1.3  | A              |

### 2.1 WorktreeManager

- `create_worktree(project_path, task_id, branch_name) -> Result<PathBuf>`
- `remove_worktree(worktree_path) -> Result<()>`
- `list_worktrees(project_path) -> Result<Vec<Worktree>>`

### 2.2 SessionManager

**Simplified:** Session now includes execution, no separate ExecutionProcess.

- `create_session(task_id, project_path, prompt, executor, branch_name) -> Result<Session>`
  - Creates worktree + spawns agent in one call
  - Returns unified Session with both worktree and execution info
- `stop_session(session_id) -> Result<()>`
  - Sends interrupt to agent, updates status to Killed
- `cleanup_session(session_id, delete_branch) -> Result<()>`
  - Removes worktree, optionally deletes branch
- `get_logs(session_id) -> Result<Vec<LogEntry>>`
  - Direct log access (no join needed)
- `subscribe_logs(session_id) -> impl Stream<Item = LogMsg>`
  - Live log streaming from MsgStore

### 2.3 Executor Trait

```rust
#[async_trait]
pub trait Executor: Send + Sync {
    /// Spawn agent server and return child process handle
    async fn spawn(
        &self,
        working_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild>;

    /// Spawn follow-up in existing session
    async fn spawn_follow_up(
        &self,
        working_dir: &Path,
        prompt: &str,
        session_id: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild>;

    /// Normalize raw logs into structured entries
    fn normalize_logs(&self, msg_store: Arc<MsgStore>, worktree_path: &Path);
}

pub struct SpawnedChild {
    pub child: tokio::process::Child,
    /// Executor → Container: signals when executor wants to exit
    pub exit_signal: Option<tokio::sync::oneshot::Receiver<()>>,
    /// Container → Executor: signals when container wants to interrupt
    pub interrupt_sender: Option<tokio::sync::mpsc::Sender<()>>,
}

pub struct ExecutionEnv {
    pub repo_paths: Vec<PathBuf>,
    pub env_vars: HashMap<String, String>,
}
```

## Phase 2.5: OpenCode Executor

| Task                                            | Status | Deps | Parallel Group |
| ----------------------------------------------- | ------ | ---- | -------------- |
| 2.4 executor/msg_store.rs - MsgStore            | [ ]    | 2.3  | B              |
| 2.5 executor/opencode/types.rs - Event types    | [ ]    | 2.3  | B              |
| 2.6 executor/opencode/sdk.rs - HTTP/SSE client  | [ ]    | 2.4  | -              |
| 2.7 executor/opencode.rs - OpenCodeExecutor     | [ ]    | 2.6  | -              |
| 2.8 executor/opencode/normalize.rs - Log parser | [ ]    | 2.5  | B              |

### 2.4 MsgStore

```rust
pub struct MsgStore {
    messages: RwLock<Vec<LogMsg>>,
    notify: broadcast::Sender<()>,
}

pub enum LogMsg {
    Stdout(String),
    Stderr(String),
    Event(SdkEvent),
    SessionId(String),
    Finished,
}
```

### 2.5 OpenCode Event Types

```rust
pub enum SdkEvent {
    SessionStatus { status: SessionStatus },
    MessageUpdated { content: String },
    ToolPart { tool_name: String, call_id: String, state: ToolState },
    PermissionAsked { tool_call_id: String, tool_name: String },
}

pub enum SessionStatus { Idle, Busy, Error }
pub enum ToolState { Pending, Running, Completed, Failed }
```

### 2.6 OpenCode SDK

HTTP client for OpenCode server (reference: vibe-kanban opencode/sdk.rs):

```rust
pub async fn run_session(
    working_dir: &Path,
    prompt: &str,
    msg_store: Arc<MsgStore>,
    interrupt_rx: Option<mpsc::Receiver<()>>,
) -> Result<oneshot::Receiver<()>>;

pub async fn wait_for_health(base_url: &str, timeout: Duration) -> Result<()>;

pub async fn create_session(base_url: &str, directory: &Path) -> Result<String>;

pub async fn fork_session(base_url: &str, session_id: &str) -> Result<String>;

pub async fn send_prompt(base_url: &str, session_id: &str, prompt: &str) -> Result<()>;

pub async fn connect_event_stream(base_url: &str) -> Result<impl Stream<Item = SdkEvent>>;

pub async fn abort_session(base_url: &str, session_id: &str) -> Result<()>;
```

**Event Stream Loop:**

1. Connect SSE to `/event`
2. Parse events as JSON (one per line)
3. Push to MsgStore
4. Handle approvals if needed
5. Wait for `session.idle` status

### 2.7 OpenCodeExecutor

**Reference: vibe-kanban opencode.rs**

```rust
pub struct OpenCodeExecutor {
    pub model: Option<String>,
    pub auto_approve: bool,
}

impl Executor for OpenCodeExecutor {
    async fn spawn(&self, working_dir: &Path, prompt: &str, env: &ExecutionEnv)
        -> Result<SpawnedChild> {
        // 1. Spawn server: `npx -y opencode-ai serve --hostname 127.0.0.1 --port 0`
        // 2. Read stdout to capture server URL (timeout: 180s)
        // 3. Parse URL from line like "Server started at http://127.0.0.1:xxxxx"
        // 4. Create MsgStore for logs
        // 5. Create interrupt channel
        // 6. Spawn sdk::run_session() in background
        // 7. Return SpawnedChild with process handle + channels
    }
}
```

**Flow:**

1. `tokio::process::Command::new("npx")` with args `["-y", "opencode-ai", "serve", ...]`
2. Spawn with stdout piped
3. Read lines until we see server URL
4. Spawn event listener task
5. Create session via HTTP
6. Send prompt
7. Stream events to MsgStore
8. Wait for idle or handle interrupt

## Phase 3: UI Components (Parallel)

> Tasks 3.1-3.4 can run in **parallel** after ui/mod.rs setup

| Task                                              | Status | Deps     | Parallel Group |
| ------------------------------------------------- | ------ | -------- | -------------- |
| 3.0 ui/mod.rs - UI module setup                   | [ ]    | 1.1      | -              |
| 3.1 ui/card.rs - Task card component              | [ ]    | 3.0      | C              |
| 3.2 ui/column.rs - Kanban column component        | [ ]    | 3.0      | C              |
| 3.3 ui/board.rs - Kanban board view               | [ ]    | 3.1, 3.2 | -              |
| 3.4 ui/session_panel.rs - Session/execution panel | [ ]    | 3.0      | C              |

## Phase 4: App Integration

| Task                                              | Status | Deps                    |
| ------------------------------------------------- | ------ | ----------------------- |
| 4.1 app.rs - Application state & message handling | [ ]    | 2.1, 2.2, 2.7, 3.3, 3.4 |
| 4.2 main.rs - Entry point, eframe setup           | [ ]    | 4.1                     |
| 4.3 lib.rs - Public API exports                   | [ ]    | 4.1                     |

## Phase 5: Request Actions

| Task                                                            | Status | Deps | Parallel Group |
| --------------------------------------------------------------- | ------ | ---- | -------------- |
| 5.1 Project CRUD (List, Get, Create, Update, Delete)            | [ ]    | 4.1  | D              |
| 5.2 Task CRUD (List, Get, Create, Update, Delete)               | [ ]    | 4.1  | D              |
| 5.3 Session actions (List, Get, Create, Stop, GetLogs, Cleanup) | [ ]    | 4.1  | D              |

## Phase 6: Worktree Lifecycle Integration

| Task                                              | Status | Deps     |
| ------------------------------------------------- | ------ | -------- |
| 6.1 Task start → CreateSession (worktree + agent) | [ ]    | 5.2, 5.3 |
| 6.2 Session complete → Push/PR flow               | [ ]    | 6.1      |
| 6.3 Cleanup → CleanupSession (remove worktree)    | [ ]    | 6.2      |

```rust
// 6.1 - Single action creates worktree + spawns agent
let session = session_manager.create_session(
    task_id,
    project_path,
    prompt,
    &opencode_executor,
    Some(branch_name),
).await?;
// → Creates worktree
// → Spawns agent
// → Returns Session with status=Running

// 6.2 - Agent completes, session updated
// (automatic when agent exits)

// 6.3 - Cleanup worktree
session_manager.cleanup_session(session.id, delete_branch).await?;
```

## Dependency Graph

```
Phase 1 (Sequential)
    │
    ▼
┌───┴───┬───────────┐
▼       ▼           ▼
2.1     2.2         2.3      ← Parallel Group A (Managers + Trait)
│       │           │         (2.2 simplified - no ExecutionProcess)
│       │       ┌───┼───┐
│       │       ▼   ▼   ▼
│       │      2.4 2.5 2.8   ← Parallel Group B (Executor internals)
│       │       │   │
│       │       └─┬─┘
│       │         ▼
│       │        2.6
│       │         │
│       │         ▼
│       │        2.7
│       │         │
└───┬───┴─────────┘
    │
    ▼
   3.0
    │
┌───┼───────┬
▼   ▼       ▼
3.1 3.2     3.4              ← Parallel Group C (UI components)
│   │       │
└─┬─┘       │
  ▼         │
 3.3        │
  │         │
  └────┬────┘
       ▼
      4.1
       │
   ┌───┼──────┐
   ▼   ▼      ▼
  4.2 4.3    5.1-5.3         ← Parallel Group D (CRUD - no 5.4)
                │
                ▼
              6.1-6.3 (Sequential, simplified)
```

## Reference Implementation

### Core Files

| File                                                                                           | Purpose                             |
| ---------------------------------------------------------------------------------------------- | ----------------------------------- |
| `/home/isomo/code/tools/vibe-kanban/crates/executors/src/executors/mod.rs`                     | `StandardCodingAgentExecutor` trait |
| `/home/isomo/code/tools/vibe-kanban/crates/executors/src/executors/opencode.rs`                | OpenCode executor entry point       |
| `/home/isomo/code/tools/vibe-kanban/crates/executors/src/executors/opencode/sdk.rs`            | HTTP/SSE client (920 lines)         |
| `/home/isomo/code/tools/vibe-kanban/crates/executors/src/executors/opencode/types.rs`          | Event types                         |
| `/home/isomo/code/tools/vibe-kanban/crates/executors/src/executors/opencode/normalize_logs.rs` | Log normalization                   |

### Key Patterns

1. **Server Mode**: Spawn `npx opencode-ai serve`, capture URL from stdout
2. **HTTP Client**: Use `reqwest` + `reqwest-eventsource` for SSE
3. **Event Stream**: Background task reads SSE, pushes to MsgStore
4. **Interruption**: `mpsc::Sender<()>` for graceful shutdown
5. **Exit Signal**: `oneshot::Receiver<()>` when executor finishes
6. **MsgStore**: In-memory buffer with `broadcast::Sender` for subscribers

### OpenCode Server Commands

```bash
# Initial session
npx -y opencode-ai serve --hostname 127.0.0.1 --port 0

# Server outputs (capture this):
Server started at http://127.0.0.1:54321
```

### HTTP API Flow

```
1. GET  /global/health           → Wait for ready
2. POST /session?directory=...   → Create session, get session_id
3. GET  /event                   → SSE stream (spawn listener)
4. POST /session/{id}/message    → Send prompt
5. Wait for event: session.idle  → Done
6. POST /session/{id}/abort      → (if interrupted)
```
