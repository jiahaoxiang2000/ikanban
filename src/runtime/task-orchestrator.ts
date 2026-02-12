import type { ConversationSessionMeta } from "../domain/conversation";
import {
  assertTaskRuntimeInvariants,
  assertTaskStateTransition,
  transitionTaskState,
  type TaskRuntime,
  type TaskState,
} from "../domain/task";
import type { ProjectRef } from "../domain/project";
import type { ProjectRegistry } from "./project-registry";
import type {
  ConversationManager,
  PromptSubmission,
  SendInitialPromptInput,
} from "./conversation-manager";
import type {
  CleanupTaskWorktreeResult,
  ManagedWorktree,
  WorktreeCleanupPolicy,
  WorktreeManager,
} from "./worktree-manager";
import { resolveCleanupPolicy } from "./worktree-manager";
import { noopRuntimeLogger, toStructuredError, type RuntimeLogger } from "./runtime-logger";

type ProjectRegistryLike = Pick<ProjectRegistry, "getProject" | "getActiveProject">;

type WorktreeManagerLike = Pick<
  WorktreeManager,
  "createTaskWorktree" | "cleanupTaskWorktree" | "getTaskWorktreeDirectory"
>;

type ConversationManagerLike = Pick<
  ConversationManager,
  "createTaskSession" | "sendInitialPrompt" | "getTaskSessionID"
>;

export type TaskOrchestratorOptions = {
  maxConcurrent?: number;
  cleanupOnSuccess?: WorktreeCleanupPolicy;
  cleanupOnFailure?: WorktreeCleanupPolicy;
  logger?: RuntimeLogger;
};

export type RunTaskInput = {
  taskId: string;
  initialPrompt: string;
  projectId?: string;
  title?: string;
  startCommand?: string;
  model?: SendInitialPromptInput["model"];
  cleanupOnSuccess?: WorktreeCleanupPolicy;
  cleanupOnFailure?: WorktreeCleanupPolicy;
  timestamp?: number;
};

export type RunTaskResult = {
  task: TaskRuntime;
  project: ProjectRef;
  worktree: ManagedWorktree;
  session: ConversationSessionMeta;
  promptSubmission: PromptSubmission;
  cleanup?: CleanupTaskWorktreeResult;
};

export type FailedRunTaskResult = {
  task: TaskRuntime;
  project?: ProjectRef;
  worktree?: ManagedWorktree;
  session?: ConversationSessionMeta;
  cleanup?: CleanupTaskWorktreeResult;
};

export class TaskRunFailedError extends Error {
  readonly result: FailedRunTaskResult;

  constructor(message: string, result: FailedRunTaskResult) {
    super(message);
    this.name = "TaskRunFailedError";
    this.result = result;
  }
}

export type TaskOrchestratorEvent =
  | {
      type: "task.state.changed";
      task: TaskRuntime;
      from: TaskState;
      to: TaskState;
    }
  | {
      type: "task.enqueued";
      task: TaskRuntime;
      queueSize: number;
    }
  | {
      type: "task.worktree.created";
      taskId: string;
      worktree: ManagedWorktree;
    }
  | {
      type: "task.session.created";
      taskId: string;
      session: ConversationSessionMeta;
    }
  | {
      type: "task.prompt.submitted";
      taskId: string;
      prompt: PromptSubmission;
    }
  | {
      type: "task.cleanup.completed";
      taskId: string;
      cleanup: CleanupTaskWorktreeResult;
      task: TaskRuntime;
    }
  | {
      type: "task.failed";
      taskId: string;
      error: string;
      task: TaskRuntime;
    };

type QueueEntry = {
  input: RunTaskInput;
  resolve: (value: RunTaskResult) => void;
  reject: (reason?: unknown) => void;
};

type CleanupExecutionResult = {
  task: TaskRuntime;
  cleanup?: CleanupTaskWorktreeResult;
};

export class TaskOrchestrator {
  private readonly projectRegistry: ProjectRegistryLike;
  private readonly worktreeManager: WorktreeManagerLike;
  private readonly conversationManager: ConversationManagerLike;
  private readonly maxConcurrent: number;
  private readonly cleanupOnSuccess: WorktreeCleanupPolicy;
  private readonly cleanupOnFailure: WorktreeCleanupPolicy;
  private readonly logger: RuntimeLogger;
  private readonly tasksById = new Map<string, TaskRuntime>();
  private readonly taskQueue: QueueEntry[] = [];
  private readonly runningTaskIds = new Set<string>();
  private readonly listeners = new Set<(event: TaskOrchestratorEvent) => void>();

  constructor(
    dependencies: {
      projectRegistry: ProjectRegistryLike;
      worktreeManager: WorktreeManagerLike;
      conversationManager: ConversationManagerLike;
    },
    options: TaskOrchestratorOptions = {},
  ) {
    this.projectRegistry = dependencies.projectRegistry;
    this.worktreeManager = dependencies.worktreeManager;
    this.conversationManager = dependencies.conversationManager;
    this.maxConcurrent = normalizeMaxConcurrent(options.maxConcurrent);
    this.cleanupOnSuccess = resolveCleanupPolicy(options.cleanupOnSuccess, "keep");
    this.cleanupOnFailure = resolveCleanupPolicy(options.cleanupOnFailure, "keep");
    this.logger = options.logger ?? noopRuntimeLogger;
  }

  runTask(input: RunTaskInput): Promise<RunTaskResult> {
    const taskId = normalizeId(input.taskId, "Task id");
    const prompt = normalizePrompt(input.initialPrompt);
    const timestamp = normalizeTimestamp(input.timestamp ?? Date.now(), "Timestamp");
    const existingTask = this.tasksById.get(taskId);

    if (existingTask && (this.runningTaskIds.has(taskId) || existingTask.state !== "completed")) {
      throw new Error(`Task ${taskId} is already queued or running.`);
    }

    const runtime: TaskRuntime = {
      taskId,
      projectId: normalizeOptionalId(input.projectId) ?? "pending",
      state: "queued",
      createdAt: timestamp,
      updatedAt: timestamp,
    };

    assertTaskRuntimeInvariants(runtime);
    this.tasksById.set(taskId, runtime);

    return new Promise<RunTaskResult>((resolve, reject) => {
      this.taskQueue.push({
        input: {
          ...input,
          taskId,
          initialPrompt: prompt,
          timestamp,
        },
        resolve,
        reject,
      });

      this.emit({
        type: "task.enqueued",
        task: this.getTaskOrThrow(taskId),
        queueSize: this.taskQueue.length,
      });

      this.schedule();
    });
  }

  getTask(taskId: string): TaskRuntime | undefined {
    const normalizedTaskId = normalizeId(taskId, "Task id");
    return this.tasksById.get(normalizedTaskId);
  }

  listTasks(): TaskRuntime[] {
    return [...this.tasksById.values()].sort((left, right) => {
      if (left.createdAt !== right.createdAt) {
        return left.createdAt - right.createdAt;
      }

      return left.taskId.localeCompare(right.taskId);
    });
  }

  getQueueSize(): number {
    return this.taskQueue.length;
  }

  getRunningCount(): number {
    return this.runningTaskIds.size;
  }

  subscribe(listener: (event: TaskOrchestratorEvent) => void): () => void {
    this.listeners.add(listener);

    return () => {
      this.listeners.delete(listener);
    };
  }

  private schedule(): void {
    while (this.runningTaskIds.size < this.maxConcurrent && this.taskQueue.length > 0) {
      const nextEntry = this.taskQueue.shift();
      if (!nextEntry) {
        return;
      }

      const taskId = nextEntry.input.taskId;
      this.runningTaskIds.add(taskId);
      void this.executeTask(nextEntry).finally(() => {
        this.runningTaskIds.delete(taskId);
        this.schedule();
      });
    }
  }

  private async executeTask(entry: QueueEntry): Promise<void> {
    const taskId = entry.input.taskId;
    let runtime = this.getTaskOrThrow(taskId);
    let project: ProjectRef | undefined;
    let worktree: ManagedWorktree | undefined;
    let session: ConversationSessionMeta | undefined;
    let promptSubmission: PromptSubmission | undefined;
    let cleanup: CleanupTaskWorktreeResult | undefined;

    try {
      const resolvedProject = await this.resolveProject(entry.input.projectId);
      project = resolvedProject;
      runtime = this.updateTask(taskId, (current) => ({
        ...current,
        projectId: resolvedProject.id,
      }));
      runtime = this.transitionTask(taskId, "creating_worktree");

      const createdWorktree = await this.worktreeManager.createTaskWorktree({
        projectDirectory: resolvedProject.rootDirectory,
        taskId,
        startCommand: entry.input.startCommand,
        timestamp: entry.input.timestamp,
      });
      worktree = createdWorktree;
      runtime = this.updateTask(taskId, (current) => ({
        ...current,
        worktreeDirectory: createdWorktree.directory,
      }));
      this.emit({
        type: "task.worktree.created",
        taskId,
        worktree: createdWorktree,
      });

      const createdSession = await this.conversationManager.createTaskSession({
        projectId: resolvedProject.id,
        taskId,
        projectDirectory: resolvedProject.rootDirectory,
        worktreeDirectory: createdWorktree.directory,
        title: entry.input.title,
        timestamp: entry.input.timestamp,
      });
      session = createdSession;
      this.emit({
        type: "task.session.created",
        taskId,
        session: createdSession,
      });

      runtime = this.transitionTaskWithPatch(taskId, "running", {
        sessionID: createdSession.sessionID,
      });

      promptSubmission = await this.conversationManager.sendInitialPrompt({
        sessionID: createdSession.sessionID,
        prompt: entry.input.initialPrompt,
        worktreeDirectory: createdWorktree.directory,
        model: entry.input.model,
      });
      this.emit({
        type: "task.prompt.submitted",
        taskId,
        prompt: promptSubmission,
      });

      runtime = this.transitionTask(taskId, "completed");

      const cleanupResult = await this.executeCleanup({
        task: runtime,
        taskId,
        projectDirectory: resolvedProject.rootDirectory,
        policy: resolveCleanupPolicy(entry.input.cleanupOnSuccess, this.cleanupOnSuccess),
      });

      runtime = cleanupResult.task;
      cleanup = cleanupResult.cleanup;
    } catch (error) {
      const failureMessage = toErrorMessage(error);
      this.logger.log({
        level: "error",
        source: "task-orchestrator.execute",
        message: "Task execution failed.",
        context: {
          taskId,
          projectId: project?.id,
          state: runtime.state,
        },
        error: toStructuredError(error),
      });
      runtime = this.transitionTaskToFailed(taskId, failureMessage);

      const projectDirectory = project?.rootDirectory;
      if (projectDirectory) {
        const cleanupResult = await this.executeCleanup({
          task: runtime,
          taskId,
          projectDirectory,
          policy: resolveCleanupPolicy(entry.input.cleanupOnFailure, this.cleanupOnFailure),
        });

        runtime = cleanupResult.task;
        cleanup = cleanupResult.cleanup;
      }
    }

    if (runtime.state === "completed" && project && worktree && session && promptSubmission) {
      entry.resolve({
        task: runtime,
        project,
        worktree,
        session,
        promptSubmission,
        cleanup,
      });
      return;
    }

    const failedResult: FailedRunTaskResult = {
      task: runtime,
      project,
      worktree,
      session,
      cleanup,
    };

    this.emit({
      type: "task.failed",
      taskId,
      error: runtime.error ?? "Task failed.",
      task: runtime,
    });
    entry.reject(new TaskRunFailedError(runtime.error ?? "Task failed.", failedResult));
  }

  private async executeCleanup(input: {
    task: TaskRuntime;
    taskId: string;
    projectDirectory: string;
    policy: WorktreeCleanupPolicy;
  }): Promise<CleanupExecutionResult> {
    if (!input.task.worktreeDirectory) {
      return { task: input.task };
    }

    const taskBeforeCleaning =
      input.task.state === "cleaning" ? input.task : this.transitionTask(input.taskId, "cleaning");

    try {
      const cleanup = await this.worktreeManager.cleanupTaskWorktree({
        projectDirectory: input.projectDirectory,
        taskId: input.taskId,
        policy: input.policy,
        worktreeDirectory: input.task.worktreeDirectory,
      });

      const finalState = taskBeforeCleaning.error ? "failed" : "completed";
      const finalizedTask = this.transitionTask(input.taskId, finalState, {
        error: taskBeforeCleaning.error,
      });

      this.emit({
        type: "task.cleanup.completed",
        taskId: input.taskId,
        cleanup,
        task: finalizedTask,
      });

      return {
        task: finalizedTask,
        cleanup,
      };
    } catch (error) {
      this.logger.log({
        level: "error",
        source: "task-orchestrator.cleanup",
        message: "Task worktree cleanup failed.",
        context: {
          taskId: input.taskId,
          projectDirectory: input.projectDirectory,
          worktreeDirectory: input.task.worktreeDirectory,
          policy: input.policy,
        },
        error: toStructuredError(error),
      });
      const cleanupFailureMessage = `Cleanup failed: ${toErrorMessage(error)}`;
      const mergedError = taskBeforeCleaning.error
        ? `${taskBeforeCleaning.error} ${cleanupFailureMessage}`
        : cleanupFailureMessage;
      const failedTask = this.transitionTask(input.taskId, "failed", {
        error: mergedError,
      });

      return {
        task: failedTask,
      };
    }
  }

  private transitionTask(
    taskId: string,
    to: TaskState,
    options: {
      at?: number;
      error?: string;
    } = {},
  ): TaskRuntime {
    return this.transitionTaskWithPatch(taskId, to, {}, options);
  }

  private transitionTaskWithPatch(
    taskId: string,
    to: TaskState,
    patch: Partial<TaskRuntime>,
    options: {
      at?: number;
      error?: string;
    } = {},
  ): TaskRuntime {
    const currentTask = this.getTaskOrThrow(taskId);
    assertTaskStateTransition(currentTask.state, to);

    const baseTask =
      Object.keys(patch).length === 0
        ? transitionTaskState(currentTask, to, options)
        : {
            ...currentTask,
            ...patch,
            state: to,
            updatedAt: options.at ?? Date.now(),
            error: resolveTransitionError(currentTask, to, options.error),
          };

    const nextTask = {
      ...baseTask,
      taskId,
    };
    assertTaskRuntimeInvariants(nextTask);
    this.tasksById.set(taskId, nextTask);
    this.emit({
      type: "task.state.changed",
      task: nextTask,
      from: currentTask.state,
      to,
    });

    return nextTask;
  }

  private transitionTaskToFailed(taskId: string, error: string): TaskRuntime {
    const currentTask = this.getTaskOrThrow(taskId);
    if (currentTask.state === "failed") {
      return this.updateTask(taskId, (task) => ({
        ...task,
        error,
      }));
    }

    return this.transitionTask(taskId, "failed", { error });
  }

  private updateTask(taskId: string, updater: (current: TaskRuntime) => TaskRuntime): TaskRuntime {
    const currentTask = this.getTaskOrThrow(taskId);
    const nextTask = {
      ...updater(currentTask),
      taskId,
      updatedAt: Date.now(),
    };

    assertTaskRuntimeInvariants(nextTask);
    this.tasksById.set(taskId, nextTask);

    return nextTask;
  }

  private getTaskOrThrow(taskId: string): TaskRuntime {
    const task = this.tasksById.get(taskId);
    if (!task) {
      throw new Error(`Task not found: ${taskId}`);
    }

    return task;
  }

  private async resolveProject(projectId?: string): Promise<ProjectRef> {
    if (projectId) {
      const project = await this.projectRegistry.getProject(projectId);
      if (!project) {
        throw new Error(`Unknown project id: ${projectId}`);
      }

      return project;
    }

    const activeProject = await this.projectRegistry.getActiveProject();
    if (!activeProject) {
      throw new Error("No active project is selected.");
    }

    return activeProject;
  }

  private emit(event: TaskOrchestratorEvent): void {
    for (const listener of this.listeners) {
      try {
        listener(event);
      } catch (error) {
        this.logger.log({
          level: "error",
          source: "task-orchestrator.listener",
          message: "Task orchestrator listener threw.",
          context: {
            eventType: event.type,
          },
          error: toStructuredError(error),
        });
      }
    }
  }
}

function normalizeId(value: string, label: string): string {
  const normalized = value.trim();
  if (!normalized) {
    throw new Error(`${label} is required.`);
  }

  return normalized;
}

function normalizeOptionalId(value: string | undefined): string | undefined {
  if (!value) {
    return undefined;
  }

  const normalized = value.trim();
  return normalized.length > 0 ? normalized : undefined;
}

function normalizePrompt(prompt: string): string {
  const normalizedPrompt = prompt.trim();
  if (!normalizedPrompt) {
    throw new Error("Initial prompt is required.");
  }

  return normalizedPrompt;
}

function normalizeTimestamp(value: number, label: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${label} must be a positive finite number.`);
  }

  return Math.floor(value);
}

function normalizeMaxConcurrent(value: number | undefined): number {
  if (value === undefined) {
    return 2;
  }

  if (!Number.isInteger(value) || value <= 0) {
    throw new Error("maxConcurrent must be a positive integer.");
  }

  return value;
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Unknown orchestrator error";
}

function resolveTransitionError(
  previous: TaskRuntime,
  nextState: TaskState,
  explicitError?: string,
): string | undefined {
  if (typeof explicitError === "string") {
    return explicitError;
  }

  if (nextState === "failed") {
    return previous.error ?? "Task failed.";
  }

  if (nextState === "cleaning" && previous.state === "failed") {
    return previous.error;
  }

  return undefined;
}
