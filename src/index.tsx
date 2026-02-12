#!/usr/bin/env bun

import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { render } from "ink";

import { App } from "./app/App";
import { ConversationManager } from "./runtime/conversation-manager";
import { RuntimeEventBus } from "./runtime/event-bus";
import { OpenCodeRuntime } from "./runtime/opencode-runtime";
import { ProjectRegistry } from "./runtime/project-registry";
import type { RuntimeLogger, RuntimeLogRecord } from "./runtime/runtime-logger";
import { TaskRegistry } from "./runtime/task-registry";
import { TaskOrchestrator } from "./runtime/task-orchestrator";
import { WorktreeManager } from "./runtime/worktree-manager";

const eventBus = new RuntimeEventBus();
const logger = createEventBusLogger(eventBus);
const runtime = new OpenCodeRuntime({ logger });
const projectRegistry = new ProjectRegistry({
  stateFilePath: resolve(join(homedir(), ".ikanban", "projects.json")),
});
const taskRegistry = new TaskRegistry({
  stateFilePath: resolve(join(homedir(), ".ikanban", "tasks.json")),
});
const worktreeManager = new WorktreeManager(runtime);
const conversationManager = new ConversationManager(runtime);
const orchestrator = new TaskOrchestrator({
  projectRegistry,
  taskRegistry,
  worktreeManager,
  conversationManager,
}, {
  logger,
});

render(
  <App
    services={{
      runtime,
      projectRegistry,
      orchestrator,
      worktreeManager,
      eventBus,
    }}
    defaultProjectDirectory={process.cwd()}
  />,
);

function createEventBusLogger(eventBus: RuntimeEventBus): RuntimeLogger {
  return {
    log(record: RuntimeLogRecord): void {
      eventBus.emit("log.appended", {
        level: record.level,
        message: record.message,
        source: record.source,
        eventType: "runtime.log",
        raw: {
          context: record.context,
          error: record.error,
        },
      });
    },
  };
}
