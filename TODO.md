# iKanban TODO

## ikanban-core

### Core-1: MVP Server (Completed)

- [x] Setup dependencies (axum, tokio, sqlx, serde, uuid)
- [x] Database layer (SQLite, Project/Task models & migrations)
- [x] HTTP API routes (health, projects CRUD, tasks CRUD)
- [x] WebSocket support (project/task streams)
- [x] Event broadcasting (in-memory bus, SSE endpoint)

### Core-2: Extended Project Model

- [x] **Core-2.1**: Add `repo_path` field (migration, struct, CRUD)
- [x] **Core-2.2**: Add `archived`/`pinned` flags (migration, struct, CRUD)
- [x] **Core-2.3**: Add `find_most_active()` query
- [x] **Core-2.4**: Implement `ProjectWithStatus` view struct + API endpoint

### Core-3: Extended Task Model

- [ ] **Core-3.1**: Add `branch` and `working_dir` fields
- [ ] **Core-3.2**: Add `InReview` and `Cancelled` status variants
- [ ] **Core-3.3**: Add `parent_task_id` for subtasks
- [ ] **Core-3.4**: Implement `TaskWithSessionStatus` view struct + API endpoint

### Core-4: Session Management

- [ ] **Core-4.1**: Create Session model (`models/session.rs`)
- [ ] **Core-4.2**: Create sessions table migration
- [ ] **Core-4.3**: Implement Session CRUD methods
- [ ] **Core-4.4**: Create Session API routes (`routes/sessions.rs`)

### Core-5: Execution Process System

- [ ] **Core-5.1**: Create ExecutionProcess model (`models/execution.rs`)
- [ ] **Core-5.2**: Create execution_processes table migration
- [ ] **Core-5.3**: Implement ExecutionProcess CRUD methods
- [ ] **Core-5.4**: Create ExecutionProcessLogs model
- [ ] **Core-5.5**: Create Execution API routes (`routes/executions.rs`)

### Core-6: CodingAgentTurn Entity

- [ ] **Core-6.1**: Create CodingAgentTurn model
- [ ] **Core-6.2**: Create coding_agent_turns table migration
- [ ] **Core-6.3**: Implement CodingAgentTurn methods

### Core-7: Merge Tracking

- [ ] **Core-7.1**: Create Merge models (DirectMerge, PrMerge)
- [ ] **Core-7.2**: Create merges table migration
- [ ] **Core-7.3**: Implement Merge CRUD methods
- [ ] **Core-7.4**: Create Merge API routes (`routes/merges.rs`)

### Core-8: Real-time & WebSocket Enhancements

- [ ] **Core-8.1**: Enhance event types (Session, Execution, Merge events)
- [ ] **Core-8.2**: Implement log streaming via WebSocket
- [ ] **Core-8.3**: PR monitoring background service

### Core-9: Additional Features

- [ ] **Core-9.1**: Tag/Template system
- [ ] **Core-9.2**: Image attachments
- [ ] **Core-9.3**: Scratch/Draft entity

### Core-10: Testing & Polish

- [ ] **Core-10.1**: Integration tests
- [ ] **Core-10.2**: Error handling improvements
- [ ] **Core-10.3**: Graceful shutdown

---

## ikanban-tui

### TUI-1: MVP Client (Completed)

- [x] Setup dependencies (ratatui, crossterm, reqwest, tokio-tungstenite)
- [x] HTTP client for REST API (`api.rs`)
- [x] TUI components (project list, task board)
- [x] Keyboard navigation, input popup

### TUI-2: WebSocket Integration

- [ ] **TUI-2.1**: Create WebSocket client module (`ws.rs`)
- [ ] **TUI-2.2**: Integrate WebSocket with App state
- [ ] **TUI-2.3**: Handle real-time updates

### TUI-3: Task Detail/Edit View

- [ ] **TUI-3.1**: Create TaskDetailView component
- [ ] **TUI-3.2**: Implement task editing
- [ ] **TUI-3.3**: Add task status quick-change

### TUI-4: Project Detail View

- [ ] **TUI-4.1**: Create ProjectDetailView component
- [ ] **TUI-4.2**: Implement project editing

### TUI-5: Session Management UI

- [ ] **TUI-5.1**: Create SessionListView component
- [ ] **TUI-5.2**: Implement session navigation
- [ ] **TUI-5.3**: Create new session

### TUI-6: Execution Log Viewer

- [ ] **TUI-6.1**: Create ExecutionLogView component
- [ ] **TUI-6.2**: Implement log streaming
- [ ] **TUI-6.3**: Log navigation

### TUI-7: Status Indicators & Notifications

- [ ] **TUI-7.1**: Add status indicators to project list
- [ ] **TUI-7.2**: Add status indicators to task board
- [ ] **TUI-7.3**: Notification system

### TUI-8: Enhanced Navigation & Shortcuts

- [ ] **TUI-8.1**: Global keyboard shortcuts (`?`, `r`, `/`, `q`)
- [ ] **TUI-8.2**: Project view shortcuts (`n`, `d`, `e`, `a`, `p`)
- [ ] **TUI-8.3**: Task view shortcuts (`n`, `d`, `e`, `Space`, `s`)

### TUI-9: UI Polish

- [ ] **TUI-9.1**: Confirmation dialogs
- [ ] **TUI-9.2**: Help overlay
- [ ] **TUI-9.3**: Theme support

### TUI-10: Error Handling & UX

- [ ] **TUI-10.1**: Error display
- [ ] **TUI-10.2**: Loading states
- [ ] **TUI-10.3**: Offline mode

---

## Cross-Cutting

- [ ] Unit tests for models
- [ ] Integration tests for API
- [ ] E2E tests with mock server
- [ ] CI/CD pipeline
- [ ] Docker containerization
