# iKanban Architecture (OpenCode SDK v2)

## 1. Goals

- Build a TUI Kanban application with Ink (`react` + `ink`) on Bun.
- Use **OpenCode SDK v2 only** for all AI/session/worktree operations.
- Start one OpenCode server process and support **multiple projects** by routing clients with different `directory` values.
- Run each task conversation inside an **independent git worktree**.

## 2. Non-Goals

- No dependency on OpenCode SDK v1 endpoints.
- No direct git shell orchestration for worktree lifecycle in app logic (use SDK v2 `worktree.*`).
- No web UI in this phase; TUI-first architecture only.

## 3. Core Principles

- **Single server, many scoped clients**: one server process, one client cache keyed by repo/worktree path.
- **Task isolation**: one task equals one worktree and one primary session.
- **Deterministic cleanup**: failed/cancelled task flows still remove or archive temporary worktrees.
- **Observable state machine**: every task has explicit states and transitions.

## 4. High-Level Topology

```
Ink TUI (UI + keyboard)
  -> App Controller (commands/events)
    -> OpenCodeRuntime (server lifecycle)
      -> OpenCode SDK v2 server/client
        -> OpenCode HTTP server instance

ProjectRegistry
  -> per project root directory
    -> TaskOrchestrator
      -> WorktreeManager (sdk v2 worktree API)
      -> ConversationManager (sdk v2 session/message API)
```

## 5. Main Components

### 5.1 OpenCodeRuntime

Responsibilities:

- Start/stop OpenCode once via `createOpencode()`.
- Expose `server.url` and root client.
- Create directory-scoped clients via `createOpencodeClient({ baseUrl, directory })`.
- Cache clients per directory to avoid repeated setup.

SDK v2 APIs:

- `createOpencode` (`@opencode-ai/sdk/v2`)
- `createOpencodeClient` (`@opencode-ai/sdk/v2/client`)

### 5.2 ProjectRegistry

Responsibilities:

- Register user projects (absolute repo root paths).
- Validate repo path once at registration.
- Provide active project selection to UI.
- Persist project metadata (local JSON state in future phase).

Suggested model:

```ts
type ProjectRef = {
  id: string;
  rootDirectory: string;
  name: string;
  createdAt: number;
};
```

### 5.3 WorktreeManager

Responsibilities:

- Create, list, reset, and remove task worktrees through SDK v2.
- Apply naming convention (`task-{taskId}-{timestamp}`).
- Track mapping: `taskId -> worktree.directory`.

SDK v2 APIs:

- `client.worktree.create({ directory, worktreeCreateInput })`
- `client.worktree.list({ directory })`
- `client.worktree.reset({ directory, worktreeResetInput })`
- `client.worktree.remove({ directory, worktreeRemoveInput })`

### 5.4 ConversationManager

Responsibilities:

- Create session bound to worktree directory.
- Send prompt/messages to that session.
- Stream/refresh message parts for UI rendering.

SDK v2 APIs:

- `client.session.create({ directory?, title? })`
- `client.session.prompt({ sessionID, parts, ... })`
- `client.session.messages({ sessionID })`
- `client.event.subscribe({ directory? })` (for real-time updates)

### 5.5 TaskOrchestrator

Responsibilities:

- Execute task flow end-to-end:
  1. select project
  2. create worktree
  3. create session
  4. send initial task prompt
  5. monitor status/events
  6. finalize and cleanup policy
- Enforce one independent worktree per task.

Suggested model:

```ts
type TaskRuntime = {
  taskId: string;
  projectId: string;
  state: "queued" | "creating_worktree" | "running" | "completed" | "failed" | "cleaning";
  worktreeDirectory?: string;
  sessionID?: string;
  error?: string;
  createdAt: number;
  updatedAt: number;
};
```

### 5.6 Ink UI Layer

Responsibilities:

- Keyboard-first views for projects, boards, tasks, and logs.
- Per-task panel displaying current worktree path, branch, session id, and last assistant output.
- Actions: run task, abort, retry, remove worktree, open diff (future).

## 6. Key Runtime Flows

### 6.1 Boot Flow

1. CLI starts Ink app.
2. App initializes `OpenCodeRuntime.start()`.
3. Load registered projects and select active project.

### 6.2 Run Task Flow (One Task = One Worktree)

1. User triggers `Run Task` from board item.
2. `TaskOrchestrator` requests worktree creation on project root.
3. On success, create scoped client for returned worktree directory.
4. Create session in that worktree context.
5. Send initial prompt (task description + context).
6. Subscribe to events and update UI.
7. Mark task state complete/failed.
8. Cleanup policy executes (remove worktree immediately or keep for review).

### 6.3 Cleanup Flow

- `onComplete`: configurable (`keep`, `remove`).
- `onFail`: default `keep` for debugging.
- manual cleanup command always available.

## 7. Multi-Project Strategy

- Keep one OpenCode server for the app process.
- For each project/worktree path, create a scoped client with `directory`.
- Never rely on implicit process cwd for project routing.
- All project-sensitive SDK calls include explicit path scope through client or call parameter.

## 8. Concurrency Model

- Tasks from different projects can run concurrently.
- Tasks from same project can run concurrently because each has a dedicated worktree.
- Use per-task async workers and centralized event bus.
- Throttle max concurrent tasks (configurable) to avoid model/API overload.

## 9. Error Handling

- Worktree create failure: task -> `failed`, no session created.
- Session create failure: attempt worktree cleanup if configured.
- Prompt failure: capture error, keep session/worktree for inspection.
- Server disconnect: show global banner, allow runtime restart.

## 10. Security and Safety

- Only allow configured local repo roots (deny arbitrary path traversal in UI actions).
- Keep provider keys in environment/OpenCode auth storage, never in task payload state.
- Sanitize and display absolute worktree paths before destructive actions.

## 11. Suggested Module Layout

```
src/
  app/
    App.tsx
    routes.ts
  runtime/
    opencode-runtime.ts
    project-registry.ts
    task-orchestrator.ts
    worktree-manager.ts
    conversation-manager.ts
    event-bus.ts
  domain/
    task.ts
    project.ts
    conversation.ts
  ui/
    views/
    components/
    hooks/
```

## 12. Configuration

```ts
type AppConfig = {
  opencode: {
    hostname?: string;
    port?: number;
    timeoutMs?: number;
  };
  tasks: {
    maxConcurrent: number;
    cleanupOnSuccess: "keep" | "remove";
    cleanupOnFailure: "keep" | "remove";
  };
};
```

## 13. Testing Strategy

- Unit tests for managers with mocked SDK clients.
- Integration tests with `RUN_REAL_OPENCODE_TESTS=1` for real worktree/session/prompt path.
- Add deterministic cleanup assertions for temporary worktrees.

## 14. Implementation Milestones

1. Runtime foundation: server lifecycle + directory-scoped client cache.
2. Project registry + persistence.
3. Worktree manager and task state machine.
4. Conversation manager and streaming output in Ink.
5. Board UI + keyboard actions.
6. Reliability: retries, cleanup policy, and concurrency controls.

## 15. SDK v2 Contract Requirement

All OpenCode interactions in this application must go through `@opencode-ai/sdk/v2` and `@opencode-ai/sdk/v2/client` APIs.
