# iKanban Multi-Agent Architecture

## Overview

iKanban is a Kanban board system that supports multiple AI coding agents working in parallel using git worktrees for isolation. Each task can have its own isolated workspace where an agent works independently without conflicts.

## Tech Stack

## Core Concept

```
Main Repo (e.g., /home/user/myproject)
├── Task A → Worktree A (branch: task/123-feature-a) → Agent 1
├── Task B → Worktree B (branch: task/456-feature-b) → Agent 2
├── Task C → Worktree C (branch: task/789-bugfix-c) → Agent 3
└── Task D → Worktree D (branch: task/012-refactor-d) → Agent 4
```

Each task gets its own isolated workspace (git worktree) where an agent can work independently without conflicts.

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

## Key Components

### 1. WorktreeManager

Manages git worktree operations.

```typescript
class WorktreeManager {
  async createWorktree(
    projectPath: string,
    taskId: string,
    branchName: string,
  ): Promise<string>;

  async removeWorktree(worktreePath: string): Promise<void>;

  async listWorktrees(projectPath: string): Promise<Worktree[]>;
}
```

### 2. SessionManager

Manages session lifecycle with worktree support.

```typescript
class SessionManager {
  async createWorktreeSession(
    taskId: string,
    projectPath: string,
    branchName?: string,
  ): Promise<Session>;

  async startAgent(
    sessionId: string,
    agentConfig: AgentConfig,
  ): Promise<ExecutionProcess>;

  async stopSession(sessionId: string): Promise<void>;
}
```

### 3. AgentProcessManager

Spawns and monitors agent processes using the Rust `agent-client-protocol`. This manager interfaces with the cargo-based agent client to execute agent behaviors in the isolated worktrees.

```typescript
class AgentProcessManager {
  // Spawns a new agent process using the rust agent-client-protocol
  async spawn(config: ProcessConfig): Promise<string>;

  async kill(processId: string): Promise<void>;

  async getOutput(processId: string): Promise<string[]>;
}
```
