import { describe, expect, test } from "bun:test";

import type { ProjectRef } from "../domain/project";
import {
  TaskOrchestrator,
  TaskRunFailedError,
  type RunTaskInput,
  type TaskOrchestratorEvent,
} from "./task-orchestrator";
import type { RuntimeLogger } from "./runtime-logger";

type Deferred<T> = {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
};

type HarnessOptions = {
  maxConcurrent?: number;
  sessionFailureForTaskId?: string;
  cleanupThrows?: boolean;
  cleanupOnSuccess?: "keep" | "remove";
  cleanupOnFailure?: "keep" | "remove";
  logger?: RuntimeLogger;
};

function createDeferred<T>(): Deferred<T> {
  let resolve: (value: T) => void = () => {};
  let reject: (reason?: unknown) => void = () => {};
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

function createProjectRef(id: string, rootDirectory: string): ProjectRef {
  return {
    id,
    name: `Project ${id}`,
    rootDirectory,
    createdAt: 1700000000000,
  };
}

function createHarness(options: HarnessOptions = {}) {
  const calls = {
    getProject: [] as Array<string>,
    getActiveProject: 0,
    createTaskWorktree: [] as Array<string>,
    createTaskSession: [] as Array<string>,
    sendInitialPrompt: [] as Array<string>,
    cleanupTaskWorktree: [] as Array<{ taskId: string; policy: "keep" | "remove" }>,
  };

  const project = createProjectRef("proj-1", "/tmp/proj-1");

  const projectRegistry = {
    async getProject(projectId: string) {
      calls.getProject.push(projectId);
      return projectId === project.id ? project : undefined;
    },
    async getActiveProject() {
      calls.getActiveProject += 1;
      return project;
    },
  };

  const worktreeManager = {
    async createTaskWorktree(input: { taskId: string }) {
      calls.createTaskWorktree.push(input.taskId);
      return {
        taskId: input.taskId,
        projectDirectory: "/tmp/proj-1",
        createdAt: 1700000001000,
        name: `task-${input.taskId}-1700000001000`,
        branch: `task/${input.taskId}`,
        directory: `/tmp/proj-1/.worktrees/${input.taskId}`,
      };
    },
    async cleanupTaskWorktree(input: { taskId: string; policy: "keep" | "remove" }) {
      calls.cleanupTaskWorktree.push({
        taskId: input.taskId,
        policy: input.policy,
      });

      if (options.cleanupThrows) {
        throw new Error("cleanup boom");
      }

      return {
        policy: input.policy,
        taskId: input.taskId,
        worktreeDirectory: `/tmp/proj-1/.worktrees/${input.taskId}`,
        removed: input.policy === "remove",
      };
    },
    getTaskWorktreeDirectory() {
      return undefined;
    },
  };

  const conversationManager = {
    async createTaskSession(input: { taskId: string }) {
      calls.createTaskSession.push(input.taskId);
      if (options.sessionFailureForTaskId === input.taskId) {
        throw new Error("session create failed");
      }

      return {
        sessionID: `session-${input.taskId}`,
        projectId: "proj-1",
        taskId: input.taskId,
        directory: `/tmp/proj-1/.worktrees/${input.taskId}`,
        title: `Task ${input.taskId}`,
        createdAt: 1700000002000,
        updatedAt: 1700000002000,
      };
    },
    async sendInitialPrompt(input: { sessionID: string; prompt: string }) {
      calls.sendInitialPrompt.push(input.sessionID);
      return {
        sessionID: input.sessionID,
        prompt: input.prompt,
        submittedAt: 1700000003000,
      };
    },
    getTaskSessionID() {
      return undefined;
    },
  };

  const orchestrator = new TaskOrchestrator(
    {
      projectRegistry,
      worktreeManager,
      conversationManager,
    },
    {
      maxConcurrent: options.maxConcurrent,
      cleanupOnSuccess: options.cleanupOnSuccess,
      cleanupOnFailure: options.cleanupOnFailure,
      logger: options.logger,
    },
  );

  return {
    orchestrator,
    calls,
  };
}

describe("TaskOrchestrator run flow", () => {
  test("runs project -> worktree -> session -> prompt and completes cleanup", async () => {
    const { orchestrator, calls } = createHarness({
      cleanupOnSuccess: "remove",
    });
    const events: TaskOrchestratorEvent[] = [];
    const unsubscribe = orchestrator.subscribe((event) => {
      events.push(event);
    });

    const result = await orchestrator.runTask({
      taskId: "task-1",
      initialPrompt: "Implement feature",
    });
    unsubscribe();

    expect(result.task.state).toBe("completed");
    expect(result.project.id).toBe("proj-1");
    expect(result.worktree.directory).toBe("/tmp/proj-1/.worktrees/task-1");
    expect(result.session.sessionID).toBe("session-task-1");
    expect(result.promptSubmission.prompt).toBe("Implement feature");
    expect(result.cleanup).toEqual({
      policy: "remove",
      taskId: "task-1",
      worktreeDirectory: "/tmp/proj-1/.worktrees/task-1",
      removed: true,
    });

    expect(calls.getActiveProject).toBe(1);
    expect(calls.createTaskWorktree).toEqual(["task-1"]);
    expect(calls.createTaskSession).toEqual(["task-1"]);
    expect(calls.sendInitialPrompt).toEqual(["session-task-1"]);
    expect(calls.cleanupTaskWorktree).toEqual([{ taskId: "task-1", policy: "remove" }]);

    const stateTransitions = events
      .filter((event) => event.type === "task.state.changed")
      .map((event) => `${event.from}->${event.to}`);

    expect(stateTransitions).toEqual([
      "queued->creating_worktree",
      "creating_worktree->running",
      "running->completed",
      "completed->cleaning",
      "cleaning->completed",
    ]);
  });

  test("transitions to failed and still runs configured failure cleanup", async () => {
    const { orchestrator, calls } = createHarness({
      sessionFailureForTaskId: "task-fail",
      cleanupOnFailure: "remove",
    });

    const runPromise = orchestrator.runTask({
      taskId: "task-fail",
      initialPrompt: "Do work",
    });

    await expect(runPromise).rejects.toBeInstanceOf(TaskRunFailedError);
    const runtime = orchestrator.getTask("task-fail");

    expect(runtime?.state).toBe("failed");
    expect(runtime?.error).toContain("session create failed");
    expect(calls.createTaskWorktree).toEqual(["task-fail"]);
    expect(calls.sendInitialPrompt).toEqual([]);
    expect(calls.cleanupTaskWorktree).toEqual([{ taskId: "task-fail", policy: "remove" }]);
  });

  test("fails deterministically when cleanup errors after success", async () => {
    const { orchestrator } = createHarness({
      cleanupThrows: true,
    });

    const runPromise = orchestrator.runTask({
      taskId: "task-cleanup-fail",
      initialPrompt: "Do work",
    });

    await expect(runPromise).rejects.toBeInstanceOf(TaskRunFailedError);
    const runtime = orchestrator.getTask("task-cleanup-fail");

    expect(runtime?.state).toBe("failed");
    expect(runtime?.error).toContain("Cleanup failed: cleanup boom");
  });

  test("emits structured error log when execution fails", async () => {
    const logs: Array<{ level: string; source: string; message: string; taskId?: string }> = [];
    const { orchestrator } = createHarness({
      sessionFailureForTaskId: "task-log-fail",
      logger: {
        log(record) {
          logs.push({
            level: record.level,
            source: record.source,
            message: record.message,
            taskId: typeof record.context?.taskId === "string" ? record.context.taskId : undefined,
          });
        },
      },
    });

    await expect(
      orchestrator.runTask({
        taskId: "task-log-fail",
        initialPrompt: "Do work",
      }),
    ).rejects.toBeInstanceOf(TaskRunFailedError);

    expect(logs.some((record) => record.source === "task-orchestrator.execute")).toBe(true);
  });
});

describe("TaskOrchestrator scheduler", () => {
  test("respects maxConcurrent limit with queued tasks", async () => {
    const firstPromptGate = createDeferred<void>();
    const secondPromptGate = createDeferred<void>();
    const promptGates = [firstPromptGate, secondPromptGate];
    const calls: { createTaskWorktree: string[] } = {
      createTaskWorktree: [],
    };

    const orchestrator = new TaskOrchestrator(
      {
        projectRegistry: {
          async getProject() {
            return undefined;
          },
          async getActiveProject() {
            return createProjectRef("proj-1", "/tmp/proj-1");
          },
        },
        worktreeManager: {
          async createTaskWorktree(input: { taskId: string }) {
            calls.createTaskWorktree.push(input.taskId);
            return {
              taskId: input.taskId,
              projectDirectory: "/tmp/proj-1",
              createdAt: 1700000001000,
              name: `task-${input.taskId}-1700000001000`,
              branch: `task/${input.taskId}`,
              directory: `/tmp/proj-1/.worktrees/${input.taskId}`,
            };
          },
          async cleanupTaskWorktree(input: { taskId: string; policy: "keep" | "remove" }) {
            return {
              taskId: input.taskId,
              policy: input.policy,
              worktreeDirectory: `/tmp/proj-1/.worktrees/${input.taskId}`,
              removed: false,
            };
          },
          getTaskWorktreeDirectory() {
            return undefined;
          },
        },
        conversationManager: {
          async createTaskSession(input: { taskId: string }) {
            return {
              sessionID: `session-${input.taskId}`,
              projectId: "proj-1",
              taskId: input.taskId,
              directory: `/tmp/proj-1/.worktrees/${input.taskId}`,
              createdAt: 1700000002000,
              updatedAt: 1700000002000,
            };
          },
          async sendInitialPrompt(input: { sessionID: string; prompt: string }) {
            const gate = promptGates.shift();
            if (!gate) {
              throw new Error("missing prompt gate");
            }

            await gate.promise;
            return {
              sessionID: input.sessionID,
              prompt: input.prompt,
              submittedAt: Date.now(),
            };
          },
          getTaskSessionID() {
            return undefined;
          },
        },
      },
      {
        maxConcurrent: 1,
      },
    );

    const firstTask: RunTaskInput = {
      taskId: "task-1",
      initialPrompt: "first",
    };
    const secondTask: RunTaskInput = {
      taskId: "task-2",
      initialPrompt: "second",
    };

    const firstRun = orchestrator.runTask(firstTask);
    const secondRun = orchestrator.runTask(secondTask);

    await Bun.sleep(0);
    expect(calls.createTaskWorktree).toEqual(["task-1"]);
    expect(orchestrator.getRunningCount()).toBe(1);
    expect(orchestrator.getQueueSize()).toBe(1);

    firstPromptGate.resolve();
    await firstRun;

    await Bun.sleep(0);
    expect(calls.createTaskWorktree).toEqual(["task-1", "task-2"]);

    secondPromptGate.resolve();
    await secondRun;
    await Bun.sleep(0);

    expect(orchestrator.getRunningCount()).toBe(0);
    expect(orchestrator.getQueueSize()).toBe(0);
  });
});
