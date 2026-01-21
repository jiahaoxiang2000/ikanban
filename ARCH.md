# iKanban Multi-Agent Architecture

## Overview

iKanban is a Kanban board system that supports multiple AI coding agents working in parallel using git worktrees for isolation. Each task can have its own isolated workspace where an agent works independently without conflicts.

## Core Concept

```
Main Repo (e.g., /home/user/myproject)
├── Task A → Worktree A (branch: task/123-feature-a) → Agent 1
├── Task B → Worktree B (branch: task/456-feature-b) → Agent 2
├── Task C → Worktree C (branch: task/789-bugfix-c) → Agent 3
└── Task D → Worktree D (branch: task/012-refactor-d) → Agent 4
```

Each task gets its own isolated workspace (git worktree) where an agent can work independently without conflicts.

## Technology Stack

- **Runtime**: Bun
- **Backend**: Hono (lightweight web framework)
- **Frontend**: React 18 + TypeScript + Vite
- **Database**: SQLite
- **ORM**: TBD (Drizzle ORM or TypeORM)
- **Communication**: WebSocket only (no REST API)
- **Package Manager**: Bun workspaces (monorepo)

## Project Structure

```
/home/isomo/code/tools/ikanban/
├── packages/
│   ├── server/          # Hono backend (WebSocket server)
│   │   └── src/
│   │       └── index.ts
│   ├── shared/          # Shared types & models
│   │   └── src/
│   │       └── index.ts
│   └── ui/              # React + Vite frontend
│       └── src/
│           ├── App.tsx
│           └── main.tsx
├── ARCH.md              # This file
├── AGENTS.md            # Agent rules and guidelines
├── TODO.md              # Implementation tasks
└── package.json         # Workspace configuration
```

## Data Model

### Core Entities

```typescript
// Projects (1:1 with Git Repository)
interface Project {
  id: string
  name: string
  repo_path: string        // Path to git repository
  archived: boolean
  pinned: boolean
  created_at: string
  updated_at: string
}

// Tasks (Kanban cards)
type TaskStatus = 'Todo' | 'InProgress' | 'InReview' | 'Done' | 'Cancelled'

interface Task {
  id: string
  project_id: string
  title: string
  description?: string
  status: TaskStatus
  branch?: string          // Git branch name (e.g., "task/123-auth")
  working_dir?: string     // Worktree path (e.g., "~/.ikanban/worktrees/task-123")
  parent_task_id?: string  // For subtasks
  created_at: string
  updated_at: string
}

// Sessions (execution runs)
type SessionStatus = 'Running' | 'Completed' | 'Failed' | 'Cancelled'

interface Session {
  id: string
  task_id: string
  executor: string         // Agent type (e.g., "claude-code", "aider")
  status: SessionStatus
  worktree_path?: string   // Absolute path to worktree
  branch_name?: string     // Git branch for this session
  started_at: string
  completed_at?: string
}

// Execution Processes (individual process runs)
type ExecutionProcessStatus = 'Running' | 'Completed' | 'Failed' | 'Killed'
type ExecutionProcessRunReason =
  | 'SetupScript'
  | 'CleanupScript'
  | 'CodingAgent'
  | 'DevServer'

interface ExecutionProcess {
  id: string
  session_id: string
  run_reason: ExecutionProcessRunReason
  executor_action?: string
  status: ExecutionProcessStatus
  exit_code?: number
  dropped: boolean         // Process was intentionally stopped
  working_directory?: string  // Worktree path for execution
  started_at: string
  completed_at?: string
}

// Coding Agent Turns (agent interaction tracking)
interface CodingAgentTurn {
  id: string
  execution_process_id: string
  agent_session_id?: string  // External agent session ID
  prompt: string
  summary?: string
  seen: boolean            // Marked as read by user
  created_at: string
  updated_at: string
}

// Merge Management
type MergeStatus = 'Open' | 'Merged' | 'Closed' | 'Unknown'

interface DirectMerge {
  id: string
  project_id: string
  merge_commit?: string
  target_branch: string
  created_at: string
}

interface PrMerge {
  id: string
  project_id: string
  target_branch: string
  pr_number: number
  pr_url: string
  status: MergeStatus
  merged_at?: string
  created_at: string
}
```

## Architecture Diagram

```
┌───────────────────────────────────────────────────────────────┐
│                    Web UI (React)                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ Kanban Board │  │ Running      │  │ Agent Output     │   │
│  │              │  │ Sessions     │  │ (Live logs)      │   │
│  │ Todo         │  │ Task 1 ⚙️    │  │                  │   │
│  │ InProgress   │  │ Task 3 ⚙️    │  │ > Agent working  │   │
│  │ InReview     │  │ Task 5 ⚙️    │  │ > Creating auth  │   │
│  │ Done         │  │              │  │ > Running tests  │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└──────────────────────────┬────────────────────────────────────┘
                           │ WebSocket (ws://server/api/ws)
                           ↓
┌───────────────────────────────────────────────────────────────┐
│                 iKanban Server (Hono + Bun)                   │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │          WebSocket Handler                              │ │
│  │  • ListProjects, CreateProject, UpdateProject           │ │
│  │  • ListTasks, CreateTask, UpdateTask, DeleteTask        │ │
│  │  • CreateSession, StopSession, ListSessions             │ │
│  │  • Subscribe/Unsubscribe to events                      │ │
│  └─────────────────────────────────────────────────────────┘ │
│                           │                                   │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │         Session Manager                                 │ │
│  ├─────────────────────────────────────────────────────────┤ │
│  │  createWorktreeSession(task_id):                        │ │
│  │    1. Generate branch name                              │ │
│  │    2. Create git worktree                               │ │
│  │    3. Create session record                             │ │
│  │    4. Return worktree path                              │ │
│  │                                                          │ │
│  │  startAgent(session_id):                                │ │
│  │    1. Get worktree path from session                    │ │
│  │    2. Spawn coding agent in worktree                    │ │
│  │    3. Create ExecutionProcess                           │ │
│  │    4. Stream output via WebSocket events               │ │
│  │                                                          │ │
│  │  stopSession(session_id):                               │ │
│  │    1. Kill agent process                                │ │
│  │    2. Update session status                             │ │
│  │    3. Optionally cleanup worktree                       │ │
│  └─────────────────────────────────────────────────────────┘ │
│                           │                                   │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │         Git Worktree Manager                            │ │
│  ├─────────────────────────────────────────────────────────┤ │
│  │  createWorktree(project_path, branch):                  │ │
│  │    → `git worktree add <path> -b <branch>`              │ │
│  │                                                          │ │
│  │  removeWorktree(worktree_path):                         │ │
│  │    → `git worktree remove <path>`                       │ │
│  │                                                          │ │
│  │  listWorktrees(project_path):                           │ │
│  │    → `git worktree list --porcelain`                    │ │
│  └─────────────────────────────────────────────────────────┘ │
│                           │                                   │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │         Agent Process Manager                           │ │
│  ├─────────────────────────────────────────────────────────┤ │
│  │  • Spawn agent CLI (claude-code, aider, etc.)           │ │
│  │  • Capture stdout/stderr                                │ │
│  │  • Store logs in execution_process_logs                 │ │
│  │  • Broadcast output as WebSocket events                 │ │
│  │  • Handle process lifecycle (start/stop/crash)          │ │
│  └─────────────────────────────────────────────────────────┘ │
│                           │                                   │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │         Database Layer (Drizzle/TypeORM)                │ │
│  │  • projects  • tasks  • sessions                        │ │
│  │  • execution_processes  • execution_process_logs        │ │
│  │  • coding_agent_turns                                   │ │
│  │  • direct_merges  • pr_merges                           │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────┬────────────────────────────────────┘
                           ↓
┌───────────────────────────────────────────────────────────────┐
│                   SQLite Database                             │
│              ~/.ikanban/data/db.sqlite                        │
└───────────────────────────────────────────────────────────────┘
                           ↕
┌───────────────────────────────────────────────────────────────┐
│              Git Worktrees (on filesystem)                    │
│                                                               │
│  ~/.ikanban/worktrees/                                        │
│    ├── task-101/  (git branch: task/101-feature)             │
│    ├── task-102/  (git branch: task/102-bugfix)              │
│    └── task-103/  (git branch: task/103-refactor)            │
└───────────────────────────────────────────────────────────────┘
```

## Worktree Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│  1. User Creates Task in "Todo" column                     │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  2. User Starts Task → Move to "InProgress"                │
│     • Create Session                                        │
│     • Generate branch name: `task/{task_id}-{slug}`        │
│     • Create git worktree:                                  │
│       `git worktree add ~/.ikanban/worktrees/task-{id}`    │
│     • Store worktree_path in session                        │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  3. Agent Execution (in worktree)                          │
│     • Create ExecutionProcess (run_reason: 'CodingAgent')  │
│     • Spawn agent with working_dir = worktree_path         │
│     • Agent works in isolated worktree                      │
│     • Track turns in CodingAgentTurn                       │
│     • Agent makes commits in worktree branch               │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  4. Session Complete                                        │
│     • Push branch to remote (optional)                      │
│     • Create PR or merge directly                          │
│     • Update session status: 'Completed'                   │
│     • Move task to "InReview" or "Done"                    │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│  5. Cleanup (manual or automatic)                          │
│     • Remove worktree: `git worktree remove`               │
│     • Delete local branch (optional)                       │
└─────────────────────────────────────────────────────────────┘
```

## WebSocket Protocol

### Connection

- Endpoint: `ws://server/api/ws`
- All operations use request/response pattern over single WebSocket connection
- Events are broadcast to subscribed clients

### Message Types

**Request** (client → server):

```json
{
  "type": "Request",
  "payload": {
    "id": "uuid",
    "action": "ListProjects"
  }
}
```

**Response** (server → client):

```json
{
  "type": "Response",
  "payload": {
    "id": "uuid",
    "status": "Success",
    "data": {...}
  }
}
```

**Event** (server → client, broadcast):

```json
{
  "type": "Event",
  "payload": {
    "event": "ProjectCreated",
    "payload": {...}
  }
}
```

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

**Sessions:**
- `ListSessions` - Get sessions for a task
- `GetSession` - Get session by ID
- `CreateSession` - Start new agent session
- `StopSession` - Stop running session

**Executions:**
- `ListExecutions` - Get execution processes for session
- `GetExecution` - Get execution details
- `GetExecutionLogs` - Get logs for execution

**Subscriptions:**
- `Subscribe` - Subscribe to event types
- `Unsubscribe` - Unsubscribe from events

### Worktree-Specific API

**Create session with worktree:**
```json
{
  "type": "Request",
  "payload": {
    "id": "uuid",
    "action": "CreateSession",
    "task_id": "task-123",
    "use_worktree": true,
    "branch_name": "task/123-add-auth"
  }
}
```

**Response includes worktree info:**
```json
{
  "type": "Response",
  "payload": {
    "id": "uuid",
    "status": "Success",
    "data": {
      "session_id": "session-456",
      "worktree_path": "/home/user/.ikanban/worktrees/task-123",
      "branch_name": "task/123-add-auth"
    }
  }
}
```

**Event: Agent output streaming:**
```json
{
  "type": "Event",
  "payload": {
    "event": "AgentOutput",
    "payload": {
      "execution_process_id": "exec-789",
      "output_type": "stdout",
      "content": "Creating authentication module..."
    }
  }
}
```

## Key Components

### 1. WorktreeManager

Manages git worktree operations.

```typescript
class WorktreeManager {
  async createWorktree(
    projectPath: string,
    taskId: string,
    branchName: string
  ): Promise<string>

  async removeWorktree(worktreePath: string): Promise<void>

  async listWorktrees(projectPath: string): Promise<Worktree[]>
}
```

### 2. SessionManager

Manages session lifecycle with worktree support.

```typescript
class SessionManager {
  async createWorktreeSession(
    taskId: string,
    projectPath: string,
    branchName?: string
  ): Promise<Session>

  async startAgent(
    sessionId: string,
    agentConfig: AgentConfig
  ): Promise<ExecutionProcess>

  async stopSession(sessionId: string): Promise<void>
}
```

### 3. AgentProcessManager

Spawns and monitors agent processes.

```typescript
class AgentProcessManager {
  async spawn(config: ProcessConfig): Promise<string>

  async kill(processId: string): Promise<void>

  async getOutput(processId: string): Promise<string[]>
}
```

## Parallel Execution Model

```
Project: ~/myproject (main branch)
│
├─ ~/.ikanban/worktrees/
│  ├─ task-101/  (branch: task/101-login)     ← Agent 1 working
│  │  └─ [Full project copy on separate branch]
│  │
│  ├─ task-102/  (branch: task/102-api)       ← Agent 2 working
│  │  └─ [Full project copy on separate branch]
│  │
│  ├─ task-103/  (branch: task/103-ui)        ← Agent 3 working
│  │  └─ [Full project copy on separate branch]
│  │
│  └─ task-104/  (branch: task/104-tests)     ← Agent 4 working
     └─ [Full project copy on separate branch]

All agents work independently without blocking each other!
```

## Benefits of Worktree Approach

✅ **Parallel Execution** - Multiple agents work simultaneously without conflicts
✅ **Isolation** - Each task has its own branch and workspace
✅ **No Stashing** - Agents don't interfere with each other's changes
✅ **Clean History** - Each task has its own git branch with clear commits
✅ **Easy Cleanup** - Remove worktree when task is done
✅ **Simple Model** - No complex agent orchestration or communication

## Example User Flow

```bash
# User has project: ~/myproject

# 1. Create tasks in Web UI
Task 101: "Add user authentication"
Task 102: "Build API endpoints"
Task 103: "Create admin UI"

# 2. Start all tasks in parallel
→ Session 1: worktree at ~/.ikanban/worktrees/task-101 (branch: task/101-auth)
→ Session 2: worktree at ~/.ikanban/worktrees/task-102 (branch: task/102-api)
→ Session 3: worktree at ~/.ikanban/worktrees/task-103 (branch: task/103-ui)

# 3. Agents work independently
Agent 1 in task-101: Making commits to task/101-auth
Agent 2 in task-102: Making commits to task/102-api
Agent 3 in task-103: Making commits to task/103-ui

# 4. Complete sessions
→ Push branches to remote
→ Create PRs or merge directly
→ Clean up worktrees

# 5. Main repo stays clean
$ cd ~/myproject
$ git branch
  main
  task/101-auth
  task/102-api
  task/103-ui
```

## Implementation Phases

### Phase 1: Foundation
- [ ] Database layer with ORM (Drizzle/TypeORM)
- [ ] Complete WebSocket request/response handlers
- [ ] Basic entity CRUD operations
- [ ] Event broadcasting system

### Phase 2: Core Features
- [ ] WorktreeManager implementation
- [ ] SessionManager with worktree support
- [ ] AgentProcessManager for spawning agents
- [ ] Database migrations and schema

### Phase 3: UI Integration
- [ ] Kanban board UI
- [ ] Task creation and management
- [ ] Session control panel
- [ ] Real-time agent output display

### Phase 4: Advanced Features
- [ ] PR creation and merge tracking
- [ ] Agent configuration and selection
- [ ] Session history and logs
- [ ] Project archiving and management

## Security Considerations

- Validate all git commands to prevent injection
- Sanitize worktree paths to prevent directory traversal
- Limit concurrent sessions to prevent resource exhaustion
- Implement proper error handling for agent crashes
- Secure WebSocket connections (WSS in production)

## Performance Considerations

- Use connection pooling for database
- Implement proper indexing on frequently queried columns
- Stream large log outputs instead of loading all into memory
- Clean up old worktrees periodically
- Implement pagination for list operations

## Future Enhancements

- Support for multiple agent types (claude-code, aider, cursor, etc.)
- Agent configuration templates
- Task dependencies and workflows
- Team collaboration features
- Metrics and analytics dashboard
- Git hooks integration for automated testing
