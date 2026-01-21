# iKanban Implementation TODO

## Phase 1: Foundation (Database & Core Infrastructure)

### Database Setup
- [ ] Choose ORM (Drizzle ORM recommended for Bun + TypeScript)
- [ ] Create database schema definitions
  - [ ] projects table
  - [ ] tasks table
  - [ ] sessions table
  - [ ] execution_processes table
  - [ ] execution_process_logs table
  - [ ] coding_agent_turns table
  - [ ] direct_merges table
  - [ ] pr_merges table
- [ ] Implement database migration system
- [ ] Create database connection manager
- [ ] Add database initialization on server start

### WebSocket Infrastructure
- [ ] Implement complete WebSocket message handler in `packages/server/src/index.ts`
- [ ] Create request routing system
- [ ] Implement action handlers:
  - [ ] ListProjects, GetProject, CreateProject, UpdateProject, DeleteProject
  - [ ] ListTasks, GetTask, CreateTask, UpdateTask, DeleteTask
  - [ ] ListSessions, GetSession, CreateSession, StopSession
  - [ ] ListExecutions, GetExecution, GetExecutionLogs
  - [ ] Subscribe, Unsubscribe
- [ ] Implement event broadcasting system
- [ ] Add subscription management for clients

### Data Access Layer
- [ ] Create repository/service pattern for entities
  - [ ] ProjectRepository
  - [ ] TaskRepository
  - [ ] SessionRepository
  - [ ] ExecutionProcessRepository
  - [ ] CodingAgentTurnRepository
- [ ] Implement CRUD operations for all entities
- [ ] Add validation using Zod schemas (already in shared package)

## Phase 2: Git Worktree Integration

### WorktreeManager Service
- [ ] Create `packages/server/src/services/worktree-manager.ts`
- [ ] Implement `createWorktree(projectPath, taskId, branchName)`
  - [ ] Validate project path exists
  - [ ] Generate unique worktree path
  - [ ] Execute git worktree add command
  - [ ] Handle errors (branch conflicts, permissions, etc.)
- [ ] Implement `removeWorktree(worktreePath)`
  - [ ] Validate worktree exists
  - [ ] Execute git worktree remove command
  - [ ] Clean up filesystem if needed
- [ ] Implement `listWorktrees(projectPath)`
  - [ ] Parse `git worktree list --porcelain` output
  - [ ] Return structured worktree information
- [ ] Add worktree cleanup utilities
  - [ ] Remove stale worktrees
  - [ ] Prune orphaned worktrees

### SessionManager Service
- [ ] Create `packages/server/src/services/session-manager.ts`
- [ ] Implement `createWorktreeSession(taskId, projectPath, branchName?)`
  - [ ] Generate branch name if not provided
  - [ ] Call WorktreeManager to create worktree
  - [ ] Create session record in database
  - [ ] Update task status to InProgress
  - [ ] Broadcast SessionCreated event
- [ ] Implement `startAgent(sessionId, agentConfig)`
  - [ ] Get session and worktree info
  - [ ] Call AgentProcessManager to spawn agent
  - [ ] Create ExecutionProcess record
  - [ ] Start log streaming
  - [ ] Broadcast AgentStarted event
- [ ] Implement `stopSession(sessionId)`
  - [ ] Kill agent process
  - [ ] Update session status
  - [ ] Optionally clean up worktree
  - [ ] Broadcast SessionStopped event
- [ ] Add session state management
  - [ ] Track active sessions in memory
  - [ ] Handle session recovery on server restart

### AgentProcessManager Service
- [ ] Create `packages/server/src/services/agent-process-manager.ts`
- [ ] Implement `spawn(config: ProcessConfig)`
  - [ ] Spawn child process with Bun.spawn or Node child_process
  - [ ] Set working directory to worktree path
  - [ ] Configure environment variables
  - [ ] Store process reference
  - [ ] Return process ID
- [ ] Implement stdout/stderr streaming
  - [ ] Capture output in real-time
  - [ ] Store logs in execution_process_logs table
  - [ ] Broadcast AgentOutput events via WebSocket
  - [ ] Handle large output efficiently (chunking)
- [ ] Implement `kill(processId)`
  - [ ] Send termination signal to process
  - [ ] Clean up process reference
  - [ ] Update ExecutionProcess status
- [ ] Handle process lifecycle events
  - [ ] Process exit (success/failure)
  - [ ] Process crash
  - [ ] Process timeout
  - [ ] Update database accordingly

## Phase 3: UI Implementation

### Kanban Board Component
- [ ] Create `packages/ui/src/components/KanbanBoard.tsx`
- [ ] Implement drag-and-drop functionality
- [ ] Create columns: Todo, InProgress, InReview, Done
- [ ] Render tasks as cards
- [ ] Handle task status updates via WebSocket
- [ ] Add task creation modal/form
- [ ] Add task detail view

### Session Control Panel
- [ ] Create `packages/ui/src/components/SessionPanel.tsx`
- [ ] Display active sessions
- [ ] Show session status (Running, Completed, Failed)
- [ ] Add start/stop session controls
- [ ] Display session metadata (branch, worktree path)
- [ ] Show session history

### Agent Output Display
- [ ] Create `packages/ui/src/components/AgentOutput.tsx`
- [ ] Implement real-time log streaming
- [ ] Auto-scroll to latest output
- [ ] Add syntax highlighting for code snippets
- [ ] Filter logs by type (stdout, stderr)
- [ ] Add search/filter functionality
- [ ] Make output copyable

### Project Management
- [ ] Create `packages/ui/src/components/ProjectList.tsx`
- [ ] Display all projects
- [ ] Add project creation form
- [ ] Show project status (active tasks, running sessions)
- [ ] Add project settings/configuration
- [ ] Support project archiving

### WebSocket Client
- [ ] Create `packages/ui/src/services/websocket-client.ts`
- [ ] Implement connection management
- [ ] Handle reconnection logic
- [ ] Implement request/response pattern
- [ ] Add event subscription system
- [ ] Create React hooks for WebSocket data
  - [ ] useProjects()
  - [ ] useTasks(projectId)
  - [ ] useSessions(taskId)
  - [ ] useAgentOutput(executionProcessId)

## Phase 4: Advanced Features

### Git Integration
- [ ] Implement branch push to remote
- [ ] Create PR via GitHub/GitLab API
- [ ] Track PR status
- [ ] Handle direct merges
- [ ] Add merge conflict detection
- [ ] Support rebase workflows

### Agent Configuration
- [ ] Define agent types (claude-code, aider, cursor, etc.)
- [ ] Create agent configuration schema
- [ ] Implement agent selection UI
- [ ] Add custom agent command/args support
- [ ] Save agent preferences per project

### Error Handling & Recovery
- [ ] Handle git command failures gracefully
- [ ] Implement session recovery on server restart
- [ ] Add retry logic for transient failures
- [ ] Display user-friendly error messages
- [ ] Log errors for debugging

### Monitoring & Logging
- [ ] Add structured logging
- [ ] Track performance metrics
- [ ] Monitor active sessions
- [ ] Alert on agent failures
- [ ] Create dashboard for system health

## Phase 5: Polish & Optimization

### Performance
- [ ] Implement database indexing
- [ ] Add pagination for large lists
- [ ] Optimize WebSocket message size
- [ ] Implement log rotation
- [ ] Add caching where appropriate
- [ ] Profile and optimize hot paths

### Testing
- [ ] Write unit tests for services
- [ ] Add integration tests for WebSocket handlers
- [ ] Test worktree operations
- [ ] Test agent spawning and lifecycle
- [ ] Add E2E tests for UI workflows

### Documentation
- [ ] Write user guide
- [ ] Document API/WebSocket protocol
- [ ] Add developer setup instructions
- [ ] Create troubleshooting guide
- [ ] Document agent configuration options

### Deployment
- [ ] Create Docker setup
- [ ] Add systemd service file
- [ ] Document deployment steps
- [ ] Add environment configuration
- [ ] Set up CI/CD pipeline

## Future Enhancements (Post-MVP)

- [ ] Multi-user support with authentication
- [ ] Task dependencies and workflows
- [ ] Custom task templates
- [ ] Agent performance analytics
- [ ] Team collaboration features
- [ ] Integration with project management tools (Jira, Linear, etc.)
- [ ] Git hooks integration
- [ ] Automated testing per session
- [ ] Cost tracking for AI agent usage
- [ ] Session replay/debugging tools

## Quick Start Checklist

For rapid MVP development, focus on these core items:

1. [ ] Set up database with Drizzle ORM
2. [ ] Implement basic CRUD for projects and tasks
3. [ ] Complete WebSocket message routing
4. [ ] Build WorktreeManager service
5. [ ] Build SessionManager service
6. [ ] Build AgentProcessManager service
7. [ ] Create simple Kanban UI
8. [ ] Implement session control panel
9. [ ] Add real-time agent output display
10. [ ] Test end-to-end workflow (create task → start session → agent work → complete)
