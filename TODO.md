# iKanban Implementation TODO

## Phase 1: Project Setup [DONE]

- [x] Initialize Bun project with `package.json`
- [x] Configure `tsconfig.json` for TypeScript + JSX (Ink/React)
- [x] Install dependencies: `ink@^5`, `react@^18`, `@opencode-ai/sdk@^1.1.53`
- [x] Install dev dependencies: `typescript@^5`, `@types/react@^18`
- [x] Create project directory structure (`src/`, `src/state/`, `src/views/`, `src/components/`, `src/hooks/`, `src/agent/`)
- [x] Create entry point `src/index.tsx`

## Phase 2: State & Data Model [DONE]

- [x] Define types in `src/state/types.ts` (`IKanbanProject`, `IKanbanTask`, `TaskStatus`, `AppView`, `AppState`)
- [x] Implement store in `src/state/store.ts` (project/task CRUD, view navigation, selection state)
- [x] Implement local JSON storage at `~/.ikanban/data.json` (read/write projects and tasks)

---

## Parallel Group A (no cross-dependencies, can all run simultaneously)

### Phase 3: Agent Infrastructure

- [x] Implement `src/agent/instance.ts` — `AgentInstance` type and create/destroy helpers
  - [x] `createOpencode()` per worktree directory
  - [x] Session creation via `client.session.create()`
  - [x] Cleanup: `client.session.abort()`, `server.close()`
- [x] Implement `src/agent/registry.ts` — `Map<taskId, AgentInstance>` management
- [x] Git worktree add/remove shell commands (`git worktree add <path> -b task/<id>-<slug>`)

### Phase 4a: Keyboard Hook (depends only on Phase 2 store)

- [ ] `src/hooks/useKeyboard.ts` — vim-like keyboard navigation (`h/j/k/l`, `Enter`, `Esc`, `n/d/e/r/?`)

### Phase 6: Components (depends only on Phase 2 types)

- [x] `src/components/Board.tsx` — kanban board layout (4 columns)
- [x] `src/components/Column.tsx` — single kanban column with cards
- [x] `src/components/Card.tsx` — task card display
- [x] `src/components/LogPanel.tsx` — side panel for SDK event logs (stdout, events)
- [x] `src/components/Input.tsx` — text input for session prompts

---

## Parallel Group B (depends on Group A completing)

### Phase 4b: Session & Agent Hooks (depends on Phase 3 agent infra)

- [ ] `src/hooks/useSession.ts` — wraps `client.session.*` + `client.event.subscribe()` for real-time updates
  - [ ] Handle `EventSessionStatus` (idle/busy/retry)
  - [ ] Handle `EventMessageUpdated`, `EventMessagePartUpdated` (streaming text + tool calls)
  - [ ] Handle `EventPermissionUpdated` / `EventPermissionReplied`
  - [ ] Handle `EventTodoUpdated`, `EventSessionCreated`, `EventSessionUpdated`, `EventSessionError`, `EventFileEdited`
- [ ] `src/hooks/useAgent.ts` — manages `createOpencode()` lifecycle per task

### Phase 5a: ProjectView + TaskView (depends on Phase 4a keyboard + Phase 6 components)

- [ ] `src/views/ProjectView.tsx` — project list with `[n]` new, `[d]` delete, `[Enter/l]` open
- [ ] `src/views/TaskView.tsx` — kanban board with 4 columns (Todo, InProgress, InReview, Done)
  - [ ] Column navigation with `h/l`, card navigation with `j/k`
  - [ ] `[n]` new task, `[e]` edit, `[d]` delete, `[Enter/l]` open session

---

## Parallel Group C (depends on Group B completing)

### Phase 5b: SessionView (depends on Phase 4b hooks + Phase 6 components)

- [ ] `src/views/SessionView.tsx` — agent interaction view
  - [ ] Display agent messages (User, Agent, Tool parts)
  - [ ] Text input for sending prompts via `client.session.prompt()`
  - [ ] `[Ctrl+C]` stop agent via `client.session.abort()`
  - [ ] `[L]` toggle log panel

### Phase 7: App Shell (depends on Phase 5a views)

- [ ] `src/app.tsx` — root component with view state machine (Project → Task → Session)
- [ ] Wire up view transitions: `l/Enter` to drill in, `h/Esc` to go back
- [ ] Focus management (`selectedIndex`, `columnIndex`, `showLogs`, `inputFocused`)

---

## Sequential (depends on all above)

### Phase 8: Worktree Lifecycle Integration

- [ ] Start task: create worktree → create opencode instance → create session → send initial prompt → move to InProgress
- [ ] Monitor execution: subscribe to SSE events → render streaming messages → handle permissions
- [ ] Complete task: detect idle status → show diff via `client.session.diff()` → move to InReview/Done
- [ ] Follow-up: send additional prompts or fork session
- [ ] Cleanup: abort session → close server → remove worktree

### Phase 9: Polish

- [ ] Help overlay (`?` key)
- [ ] Error handling for SDK failures, git worktree errors
- [ ] Graceful shutdown (cleanup all agent instances on exit)
- [ ] Refresh (`r` key) to re-fetch project/task/session state
