# TODO - Parallel Run Plan (Based on ARCH.md)

This plan breaks implementation into parallel tracks so multiple contributors/agents can work at the same time while staying aligned to `ARCH.md`.

## Parallel Tracks

## Track A - Runtime Foundation

- [ ] Implement `src/runtime/opencode-runtime.ts`
- [ ] Add single-server lifecycle (`start`, `stop`, restart)
- [ ] Add scoped client cache by `directory`
- [ ] Add tests for server start/stop and client cache reuse

## Track B - Domain Models

- [ ] Create `src/domain/project.ts` (`ProjectRef`)
- [ ] Create `src/domain/task.ts` (`TaskRuntime`, state transitions)
- [ ] Create `src/domain/conversation.ts` (session/message metadata)
- [ ] Add validation helpers for project/task invariants

## Track C - Project Registry

- [ ] Implement `src/runtime/project-registry.ts`
- [ ] Add project add/remove/list/select operations
- [ ] Validate absolute repo root paths
- [ ] Add persistence format (JSON file) and repository tests

## Track D - Worktree Management

- [ ] Implement `src/runtime/worktree-manager.ts`
- [ ] Add `create/list/reset/remove` via SDK v2 only
- [ ] Enforce naming policy `task-{taskId}-{timestamp}`
- [ ] Add cleanup policy helpers (`keep` / `remove`)
- [ ] Add integration tests for real worktree lifecycle

## Track E - Conversation Management

- [ ] Implement `src/runtime/conversation-manager.ts`
- [ ] Add session creation in worktree-scoped directory
- [ ] Add initial prompt + follow-up prompt APIs
- [ ] Add message listing and normalization for UI rendering
- [ ] Add event subscription wrapper for real-time updates

## Track F - Task Orchestration (Core Parallel Runner)

- [ ] Implement `src/runtime/task-orchestrator.ts`
- [ ] Add end-to-end flow: project -> worktree -> session -> prompt
- [ ] Add per-task state machine transitions
- [ ] Add concurrent task scheduler with max concurrency config
- [ ] Add failure handling + deterministic cleanup behavior

## Track G - Event Bus and State Updates

- [ ] Implement `src/runtime/event-bus.ts`
- [ ] Add typed events for task/worktree/session lifecycle
- [ ] Add fan-out for UI updates and logs
- [ ] Add tests for ordering and listener cleanup

## Track H - Ink UI (TUI)

- [ ] Scaffold `src/app/App.tsx` and `src/app/routes.ts`
- [ ] Build views: project selector, task board, task detail/log panel
- [ ] Add keyboard actions: run, abort, retry, cleanup worktree
- [ ] Bind UI to orchestrator and event bus
- [ ] Add loading/error states and status banners

## Cross-Cutting

- [x] Add `AppConfig` loader (opencode + task concurrency + cleanup policy)
- [x] Add structured logging for runtime/orchestrator errors
- [x] Add guardrails for allowed project paths only
- [x] Ensure all OpenCode calls use `@opencode-ai/sdk/v2` APIs

## Milestone Plan

### M1 (Foundation)

- [ ] Track A complete
- [ ] Track B complete
- [ ] Track C complete

### M2 (Execution Core)

- [ ] Track D complete
- [ ] Track E complete
- [ ] Track F complete

### M3 (Usable TUI)

- [ ] Track G complete
- [ ] Track H complete

### M4 (Hardening)

- [ ] Real integration tests stable in CI/local
- [ ] Cleanup/retry behavior validated under failure cases
- [ ] Docs updated (`README.md`, `ARCH.md`, runtime usage examples)

## Definition of Done

- [ ] Multi-project tasks run concurrently under one OpenCode server
- [ ] Every task runs in an independent worktree
- [ ] Task completion/failure follows configured cleanup policy
- [ ] TUI can start, monitor, and control task execution end-to-end
- [ ] Test suite includes unit + real integration coverage
