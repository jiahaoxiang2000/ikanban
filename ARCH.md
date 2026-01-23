# iKanban Multi-Agent Architecture

## Overview

iKanban is a Kanban board system that supports multiple AI coding agents working in parallel using git worktrees for isolation. Each task can have its own isolated workspace where an agent works independently without conflicts.

### MVP Architecture Highlights

**Simplifications for MVP:**

1. **Unified Session Model** - No separate ExecutionProcess table
   - Session includes both worktree lifecycle AND execution info
   - Logs link directly to session (no join needed)
   - 3 fewer API endpoints

2. **Vibe-kanban Executor Pattern** - No agent-client-protocol
   - Spawn agent in server mode (`npx opencode-ai serve`)
   - Connect as HTTP/SSE client
   - Standard web protocols instead of custom RPC

3. **Atomic Session Creation** - One action does it all
   - CreateSession creates worktree + spawns agent together
   - No separate StartAgent step
   - Simpler workflow, fewer states

**Result:** Clean, minimal architecture focused on core functionality.

## Key Architecture Decisions

### Executor Pattern

**Decision**: Use vibe-kanban's executor pattern instead of `agent-client-protocol` crate.

| Aspect            | agent-client-protocol   | **vibe-kanban executor** ✓            |
| ----------------- | ----------------------- | ------------------------------------- |
| **Agent Mode**    | Client mode             | **Server mode**                       |
| **Protocol**      | Custom binary/JSON RPC  | **HTTP REST + SSE**                   |
| **Dependencies**  | `agent-client-protocol` | **`reqwest` + `reqwest-eventsource`** |
| **Spawning**      | Spawn agent as client   | **Spawn `npx opencode-ai serve`**     |
| **Communication** | Bidirectional channels  | **HTTP requests + SSE stream**        |
| **Logs**          | Protocol messages       | **Stdout + SSE events**               |

### How It Works

1. **Spawn Server**: `npx -y opencode-ai serve --hostname 127.0.0.1 --port 0`
2. **Capture URL**: Parse stdout for "Server started at http://..."
3. **HTTP Client**: Create session, send prompts via REST API
4. **SSE Stream**: Background task listens to `/event` endpoint
5. **Event Processing**: Parse events (MessageUpdated, ToolPart, SessionStatus, etc.)
6. **MsgStore**: In-memory buffer with broadcast channels for UI updates
7. **Interruption**: Use `mpsc::Sender` to abort sessions gracefully

## Tech Stack

### Desktop UI Framework (Wayland Native)

**Primary: egui** - Immediate mode GUI

- Ultra-lightweight, pure Rust, no DSL or macros required
- Native Wayland support via eframe (uses wgpu/glow backend)
- Immediate mode: UI is redrawn each frame, simple mental model

### Core Dependencies

```toml
[dependencies]
# UI
eframe = "0.29"
egui = "0.29"
egui_extras = "0.29"

# Database
rusqlite = { version = "0.32", features = ["bundled"] }

# Async Runtime
tokio = { version = "1", features = ["rt-multi-thread", "process", "sync", "time", "fs"] }

# HTTP Client (for agent server communication)
reqwest = { version = "0.12", features = ["json", "stream"] }
reqwest-eventsource = "0.6"  # SSE client

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Git Operations (worktree management)
git2 = "0.19"

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
tracing = "0.1"
dirs = "5"
async-trait = "0.1"
```

### Project Structure

```
src/
├── main.rs
├── app.rs
├── ui/
│   ├── mod.rs
│   ├── board.rs
│   ├── card.rs
│   ├── column.rs
│   └── session_panel.rs
├── db/
│   ├── mod.rs
│   └── models.rs
├── worktree/
│   ├── mod.rs
│   └── manager.rs
├── session/
│   ├── mod.rs
│   └── manager.rs
├── executor/
│   ├── mod.rs           # Trait + dispatch enum
│   ├── opencode.rs      # OpenCode executor
│   ├── opencode/
│   │   ├── sdk.rs       # HTTP client for server
│   │   ├── types.rs     # Event types
│   │   └── normalize.rs # Log normalization
│   └── msg_store.rs     # In-memory log buffer
└── lib.rs
```

## Core Concept

```
Main Repo (e.g., /home/user/myproject)
├── Task A → Worktree A (branch: task/123-feature-a) → Agent 1
├── Task B → Worktree B (branch: task/456-feature-b) → Agent 2
├── Task C → Worktree C (branch: task/789-bugfix-c) → Agent 3
└── Task D → Worktree D (branch: task/012-refactor-d) → Agent 4
```

Each task gets its own isolated workspace (git worktree) where an agent can work independently without conflicts.

## Worktree Lifecycle (MVP - Simplified)

```
┌─────────────────────────────────────────────────────────────┐
│  1. User Creates Task in "Todo" column                     │
│     • INSERT INTO tasks (title, status='Todo')              │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  2. User Starts Task → CreateSession action                │
│     • Generate branch: `task/{task_id}-{slug}`              │
│     • Create worktree: `git worktree add ~/.ikanban/...`   │
│     • INSERT INTO sessions (task_id, worktree_path,         │
│       branch_name, executor_type='opencode',                │
│       status='Running', started_at=NOW)                     │
│     • Spawn agent: executor.spawn(worktree_path, prompt)    │
│     • Update task status: 'InProgress'                      │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  3. Agent Execution (in worktree)                          │
│     • Agent runs in isolated worktree                       │
│     • Logs streamed to MsgStore                             │
│     • INSERT INTO log_entries (session_id, log_type, ...)   │
│     • Agent makes commits to worktree branch                │
│     • Session status remains 'Running'                      │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  4. Session Complete                                        │
│     • Agent exits (exit_code captured)                      │
│     • UPDATE sessions SET status='Completed',               │
│       exit_code=0, finished_at=NOW                          │
│     • Optional: Push branch + create PR                     │
│     • Update task status: 'InReview' or 'Done'              │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  5. Cleanup → CleanupSession action                        │
│     • `git worktree remove {worktree_path}`                │
│     • Optional: Delete local branch                         │
│     • Session record kept for history                       │
└─────────────────────────────────────────────────────────────┘
```

**Key Simplification:**
- No separate ExecutionProcess table
- Session combines worktree lifecycle + agent execution
- Single status field tracks entire session state

## Database Schema (MVP - Simplified)

### Optimization: Merged Sessions + Executions

**Before (Complex - 4 tables):**
```
projects → tasks → sessions → execution_processes → log_entries
                      ↓              ↓
                   worktree      process info
```

**After (MVP - 3 tables):**
```
projects → tasks → sessions → log_entries
                      ↓
              worktree + process info
```

**Key Changes:**
- ❌ Removed `execution_processes` table
- ✅ Sessions include execution columns (executor_type, status, exit_code, started_at, finished_at)
- ✅ Logs link directly to `session_id` (no join needed)

**Rationale:** For MVP, one task = one session = one agent run. No need for separate execution layer.

**Query Simplification:**
```sql
-- Before (JOIN required)
SELECT l.* FROM log_entries l
JOIN execution_processes e ON l.execution_id = e.id
WHERE e.session_id = ?

-- After (direct)
SELECT * FROM log_entries WHERE session_id = ?
```

### Tables

```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL CHECK(status IN ('Todo', 'InProgress', 'InReview', 'Done')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

-- MVP: Unified Session (worktree + execution in one)
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,

    -- Worktree lifecycle
    worktree_path TEXT,
    branch_name TEXT,

    -- Execution info
    executor_type TEXT NOT NULL DEFAULT 'opencode',
    status TEXT NOT NULL CHECK(status IN ('Running', 'Completed', 'Failed', 'Killed')),
    exit_code INTEGER,

    -- Timestamps
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    finished_at DATETIME,

    FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Logs link directly to session (no intermediate execution table)
CREATE TABLE log_entries (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    log_type TEXT NOT NULL CHECK(log_type IN ('Stdout', 'Stderr', 'Event')),
    content TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX idx_tasks_project ON tasks(project_id);
CREATE INDEX idx_sessions_task ON sessions(task_id);
CREATE INDEX idx_logs_session ON log_entries(session_id);
```

### Models (Rust)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
}

// MVP: Unified Session (combines worktree + execution)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub task_id: String,

    // Worktree lifecycle
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,

    // Execution info
    pub executor_type: String,  // "opencode"
    pub status: SessionStatus,
    pub exit_code: Option<i32>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub session_id: String,  // Direct link to session (no execution table)
    pub timestamp: DateTime<Utc>,
    pub log_type: LogType,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogType {
    Stdout,
    Stderr,
    Event,
}
```

## App Actions (MVP - Simplified)

### Request Actions

**Projects:**

- `ListProjects` - Get all projects
- `GetProject` - Get project by ID
- `CreateProject` - Create new project
- `UpdateProject` - Update project details
- `DeleteProject` - Delete project

**Tasks:**

- `ListTasks` - Get tasks for a project
- `GetTask` - Get task by ID
- `CreateTask` - Create new task
- `UpdateTask` - Update task (status, description, etc.)
- `DeleteTask` - Delete task

**Sessions (MVP - includes execution info):**

- `ListSessions` - Get sessions for a task
- `GetSession` - Get session by ID (includes worktree + execution status)
- `CreateSession` - Create session + worktree + spawn agent
- `StopSession` - Stop running agent, update status to Killed
- `GetSessionLogs` - Get logs for session (stdout/stderr/events)
- `CleanupSession` - Remove worktree, optionally delete branch

**Removed (merged into Session):**

- ~~`ListExecutions`~~ - Use `ListSessions` instead
- ~~`GetExecution`~~ - Use `GetSession` instead (includes execution status)
- ~~`GetExecutionLogs`~~ - Use `GetSessionLogs` instead

**API Endpoint Count:**

| Category | Endpoints | Count |
|----------|-----------|-------|
| Projects | List, Get, Create, Update, Delete | 5 |
| Tasks | List, Get, Create, Update, Delete | 5 |
| **Sessions (MVP)** | List, Get, Create, Stop, **GetLogs**, **Cleanup** | **6** |
| ~~Executions~~ | ~~List, Get, GetLogs~~ | **0** (merged) |
| **Total** | | **16** |

**Previous Complex Design:**
- Sessions: 4 endpoints (no GetLogs, no Cleanup)
- Executions: 3 endpoints (List, Get, GetLogs)
- Total: 17 endpoints

**MVP Simplification:**
- Sessions absorb execution functionality
- Net reduction: **-1 endpoint** overall
- But more importantly: **1 fewer entity to reason about**

**Benefits:**
- 1 fewer endpoint overall
- Simpler mental model: one session = one agent run
- Direct logs query (no join needed)
- All session data in one place
- Can extend later if multi-execution sessions are needed

## Key Components

### 1. WorktreeManager

Manages git worktree operations.

```rust
impl WorktreeManager {
    pub fn create_worktree(
        &self,
        project_path: &Path,
        task_id: &str,
        branch_name: &str,
    ) -> Result<PathBuf>;

    pub fn remove_worktree(&self, worktree_path: &Path) -> Result<()>;

    pub fn list_worktrees(&self, project_path: &Path) -> Result<Vec<Worktree>>;
}
```

### 2. SessionManager (MVP - Unified)

Manages session lifecycle (worktree + execution in one).

```rust
impl SessionManager {
    /// Creates session, worktree, and spawns agent in one action
    pub async fn create_session(
        &self,
        task_id: &str,
        project_path: &Path,
        prompt: &str,
        executor: &dyn Executor,
        branch_name: Option<&str>,
    ) -> Result<Session>;

    /// Stops running agent, updates status to Killed
    pub async fn stop_session(&self, session_id: &str) -> Result<()>;

    /// Removes worktree, optionally deletes branch
    pub async fn cleanup_session(
        &self,
        session_id: &str,
        delete_branch: bool,
    ) -> Result<()>;

    /// Gets logs for session
    pub async fn get_logs(&self, session_id: &str) -> Result<Vec<LogEntry>>;

    /// Subscribes to live log stream (from MsgStore)
    pub fn subscribe_logs(&self, session_id: &str) -> impl Stream<Item = LogMsg>;
}
```

**Simplification:**
- `create_session` now does everything: worktree + spawn agent
- No separate `start_agent` method (execution is part of session)
- Direct log access (no execution layer)

### 3. Executor System

Spawns agent in **server mode** and connects as HTTP client. Based on vibe-kanban executor pattern.

#### Core Trait

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
```

#### SpawnedChild

```rust
pub struct SpawnedChild {
    pub child: tokio::process::Child,
    /// Executor → Container: signals when executor wants to exit
    pub exit_signal: Option<tokio::sync::oneshot::Receiver<()>>,
    /// Container → Executor: signals when container wants to interrupt
    pub interrupt_sender: Option<tokio::sync::mpsc::Sender<()>>,
}
```

### 4. OpenCode Executor

Spawns OpenCode agent in server mode, connects via HTTP/SSE.

```rust
pub struct OpenCodeExecutor {
    pub model: Option<String>,
    pub auto_approve: bool,
}

impl OpenCodeExecutor {
    /// Command: `npx -y opencode-ai serve --hostname 127.0.0.1 --port 0`
    /// Server outputs URL on stdout, then we connect as client
}
```

#### OpenCode Server HTTP API

```
POST   /session?directory={path}     → Create session
POST   /session/{id}/fork            → Fork session (follow-up)
POST   /session/{id}/message         → Send prompt
GET    /event                        → SSE event stream
POST   /session/{id}/abort           → Abort session
GET    /global/health                → Health check
```

#### OpenCode Events (SSE)

```rust
pub enum SdkEvent {
    SessionStatus { status: SessionStatus },  // idle, busy, error
    MessageUpdated { content: String },
    ToolPart { tool_name: String, call_id: String, state: ToolState },
    PermissionAsked { tool_call_id: String, tool_name: String },
    TodoUpdated { todos: Vec<Todo> },
}

pub enum SessionStatus {
    Idle,    // Ready for next prompt
    Busy,    // Processing
    Error,
}
```

### 5. MsgStore

In-memory log buffer with stream interface.

```rust
pub struct MsgStore {
    messages: RwLock<Vec<LogMsg>>,
    notify: tokio::sync::broadcast::Sender<()>,
}

pub enum LogMsg {
    Stdout(String),
    Stderr(String),
    Event(SdkEvent),
    SessionId(String),
    Finished,
}

impl MsgStore {
    pub fn push(&self, msg: LogMsg);
    pub fn subscribe(&self) -> impl Stream<Item = LogMsg>;
}
```

## Executor Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  1. Spawn OpenCode Server                                       │
│     `npx -y opencode-ai serve --hostname 127.0.0.1 --port 0`   │
│     Wait for server URL on stdout (timeout: 180s)               │
└─────────────────┬───────────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────────┐
│  2. Create Session                                              │
│     POST /session?directory={worktree_path}                     │
│     Returns: { session_id: "..." }                              │
└─────────────────┬───────────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────────┐
│  3. Connect Event Stream                                        │
│     GET /event (SSE)                                            │
│     Spawn background task to process events                     │
└─────────────────┬───────────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────────┐
│  4. Send Prompt                                                 │
│     POST /session/{id}/message { prompt: "..." }                │
│     Wait for SessionStatus::Idle event                          │
└─────────────────┬───────────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────────┐
│  5. Handle Events                                               │
│     • MessageUpdated → Update UI                                │
│     • ToolPart → Show tool execution                            │
│     • PermissionAsked → Handle approval (if not auto)           │
│     • SessionStatus::Idle → Done                                │
└─────────────────────────────────────────────────────────────────┘
```
