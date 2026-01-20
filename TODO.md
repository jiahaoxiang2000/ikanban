# iKanban TODO

## Completed Features

### MVP Server (Core-1)

- [x] Setup dependencies (axum, tokio, sqlx, serde, uuid)
- [x] Database layer (SQLite, Project/Task models & migrations)
- [x] HTTP API routes (health, projects CRUD, tasks CRUD)
- [x] WebSocket support (project/task streams)
- [x] Event broadcasting (in-memory bus, SSE endpoint)

### MVP Client (TUI-1)

- [x] Setup dependencies (ratatui, crossterm, reqwest, tokio-tungstenite)
- [x] HTTP client for REST API (`api.rs`)
- [x] TUI components (project list, task board)
- [x] Keyboard navigation, input popup

---

## Active Development

### Extended Project Model

- [x] Add `repo_path` field (migration, struct, CRUD)
- [x] Add `archived`/`pinned` flags (migration, struct, CRUD)
- [x] Add `find_most_active()` query
- [x] Implement `ProjectWithStatus` view struct + API endpoint

### Feature: Real-time & WebSocket Infrastructure

_Combines Core-8 and TUI-2_

- [ ] **Core**: Enhance event types (Session, Execution, Merge events)
- [ ] **Core**: Implement log streaming via WebSocket
- [ ] **Core**: PR monitoring background service
- [ ] **TUI**: Create WebSocket client module (`ws.rs`) & Integrate with App state
- [ ] **TUI**: Handle real-time updates in UI

### Feature: Extended Task Management

_Combines Core-3 and TUI-3_

- [ ] **Core**: Add `branch` and `working_dir` fields
- [ ] **Core**: Add `InReview` and `Cancelled` status variants
- [ ] **Core**: Add `parent_task_id` for subtasks
- [ ] **Core**: Implement `TaskWithSessionStatus` view struct + API endpoint
- [ ] **TUI**: Create TaskDetailView component
- [ ] **TUI**: Implement task editing & status quick-change

### Feature: Session Management

_Combines Core-4 and TUI-5_

- [ ] **Core**: Create Session model (`models/session.rs`) & Migration
- [ ] **Core**: Implement Session CRUD methods & API routes
- [ ] **TUI**: Create SessionListView component
- [ ] **TUI**: Implement session navigation & creation UI

### Feature: Execution Process & Logs

_Combines Core-5 and TUI-6_

- [ ] **Core**: Create ExecutionProcess model & Migration
- [ ] **Core**: Implement ExecutionProcess CRUD & API
- [ ] **Core**: Create ExecutionProcessLogs model
- [ ] **TUI**: Create ExecutionLogView component
- [ ] **TUI**: Implement log streaming & navigation

### Feature: Project Details (TUI)

_TUI-4 (Core support completed)_

- [ ] **TUI**: Create ProjectDetailView component
- [ ] **TUI**: Implement project editing

### Feature: Merge Tracking

_Core-7 (TUI pending)_

- [ ] **Core**: Create Merge models (DirectMerge, PrMerge) & Migration
- [ ] **Core**: Implement Merge CRUD methods & API routes

### Feature: Coding Agent History

_Core-6_

- [ ] **Core**: Create CodingAgentTurn model & Migration
- [ ] **Core**: Implement CodingAgentTurn methods

---

## Polish & Maintenance

### UI/UX Enhancements

_Combines TUI-7, TUI-8, TUI-9, TUI-10_

- [ ] **Status**: Add indicators to project list & task board
- [ ] **Nav**: Global shortcuts (`?`, `r`, `/`, `q`) & View shortcuts
- [ ] **Polish**: Confirmation dialogs, Help overlay, Theme support
- [ ] **Reliability**: Error display, Loading states, Offline mode

### Core Enhancements & Testing

_Combines Core-9, Core-10, Cross-Cutting_

- [ ] **Feat**: Tag/Template system, Image attachments, Scratch/Draft entity
- [ ] **Code**: Integration tests, Error handling improvements, Graceful shutdown
- [ ] **Infra**: CI/CD pipeline, Docker containerization, E2E tests
