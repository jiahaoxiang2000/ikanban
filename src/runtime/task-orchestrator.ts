import type { ConversationSessionMeta } from "../domain/conversation";
import type { ConversationMessageMeta } from "../domain/conversation";
import {
  assertTaskRuntimeInvariants,
  assertTaskStateTransition,
  transitionTaskState,
  type TaskRuntime,
  type TaskState,
} from "../domain/task";
import type { ProjectRef } from "../domain/project";
import type { TaskRegistry } from "./task-registry";
import type { ProjectRegistry } from "./project-registry";
import type {
  ConversationManager,
  PromptSubmission,
  SendInitialPromptInput,
} from "./conversation-manager";
import type {
  CleanupTaskWorktreeResult,
  ManagedWorktree,
  MergeTaskWorktreeResult,
  WorktreeCleanupPolicy,
  WorktreeManager,
} from "./worktree-manager";
import { resolveCleanupPolicy } from "./worktree-manager";
import { noopRuntimeLogger, toStructuredError, type RuntimeLogger } from "./runtime-logger";

type ProjectRegistryLike = Pick<ProjectRegistry, "getProject" | "getActiveProject">;
type TaskRegistryLike = Pick<TaskRegistry, "listTasks" | "upsertTask" | "removeTask">;

type WorktreeManagerLike = Pick<
  WorktreeManager,
  "createTaskWorktree" | "cleanupTaskWorktree" | "getTaskWorktreeDirectory" | "mergeTaskWorktree"
>;

type ConversationManagerLike = Pick<
  ConversationManager,
  "createTaskSession" | "sendInitialPromptAndAwaitMessages" | "sendFollowUpPromptAndAwaitMessages" | "getTaskSessionID"
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
      type: "task.session.message.received";
      taskId: string;
      sessionID: string;
      message: ConversationMessageMeta;
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
    }
  | {
      type: "task.review";
      taskId: string;
      task: TaskRuntime;
    }
  | {
      type: "task.merged";
      taskId: string;
      branch: string;
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
  private readonly taskRegistry?: TaskRegistryLike;
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
  private initialized = false;
  private initializationPromise?: Promise<void>;

  constructor(
    dependencies: {
      projectRegistry: ProjectRegistryLike;
      taskRegistry?: TaskRegistryLike;
      worktreeManager: WorktreeManagerLike;
      conversationManager: ConversationManagerLike;
    },
    options: TaskOrchestratorOptions = {},
  ) {
    this.projectRegistry = dependencies.projectRegistry;
    this.taskRegistry = dependencies.taskRegistry;
    this.worktreeManager = dependencies.worktreeManager;
    this.conversationManager = dependencies.conversationManager;
    this.maxConcurrent = normalizeMaxConcurrent(options.maxConcurrent);
    this.cleanupOnSuccess = resolveCleanupPolicy(options.cleanupOnSuccess, "keep");
    this.cleanupOnFailure = resolveCleanupPolicy(options.cleanupOnFailure, "keep");
    this.logger = options.logger ?? noopRuntimeLogger;
  }

  async initialize(): Promise<void> {
    await this.ensureInitialized();
  }

  async runTask(input: RunTaskInput): Promise<RunTaskResult> {
    await this.ensureInitialized();

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
    this.persistTask(runtime);

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

  async deleteTask(taskId: string): Promise<boolean> {
    await this.ensureInitialized();

    const normalizedTaskId = normalizeId(taskId, "Task id");
    if (this.runningTaskIds.has(normalizedTaskId)) {
      throw new Error(`Task ${normalizedTaskId} is running and cannot be deleted.`);
    }

    const queueIndex = this.taskQueue.findIndex((entry) => entry.input.taskId === normalizedTaskId);
    if (queueIndex >= 0) {
      const [queued] = this.taskQueue.splice(queueIndex, 1);
      queued?.reject(new Error(`Task ${normalizedTaskId} was deleted before execution.`));
    }

    const task = this.tasksById.get(normalizedTaskId);
    if (!task) {
      return false;
    }

    if (task.worktreeDirectory) {
      const project = await this.projectRegistry.getProject(task.projectId);
      if (project) {
        await this.worktreeManager.cleanupTaskWorktree({
          taskId: normalizedTaskId,
          projectDirectory: project.rootDirectory,
          worktreeDirectory: task.worktreeDirectory,
          policy: "remove",
        });
      }
    }

    this.tasksById.delete(normalizedTaskId);
    this.removePersistedTask(normalizedTaskId);
    return true;
  }

  async sendFollowUpPrompt(taskId: string, prompt: string): Promise<void> {
    await this.ensureInitialized();

    const normalizedTaskId = normalizeId(taskId, "Task id");
    const normalizedPrompt = normalizePrompt(prompt);
    const task = this.getTaskOrThrow(normalizedTaskId);

    if (task.state !== "review") {
      throw new Error(`Task ${normalizedTaskId} must be in review state to send a follow-up prompt (current: ${task.state}).`);
    }

    if (!task.sessionID || !task.worktreeDirectory) {
      throw new Error(`Task ${normalizedTaskId} is missing session or worktree directory.`);
    }

    const runtime = this.transitionTask(normalizedTaskId, "running");
    this.runningTaskIds.add(normalizedTaskId);

    try {
      const promptExecution = await this.conversationManager.sendFollowUpPromptAndAwaitMessages({
        sessionID: task.sessionID,
        prompt: normalizedPrompt,
        worktreeDirectory: task.worktreeDirectory,
        onMessage: (message) => {
          this.emit({
            type: "task.session.message.received",
            taskId: normalizedTaskId,
            sessionID: task.sessionID!,
            message,
          });
        },
      });

      this.emit({
        type: "task.prompt.submitted",
        taskId: normalizedTaskId,
        prompt: promptExecution.submission,
      });

      const reviewRuntime = this.transitionTask(normalizedTaskId, "review");
      this.emit({
        type: "task.review",
        taskId: normalizedTaskId,
        task: reviewRuntime,
      });
    } catch (error) {
      const failureMessage = toErrorMessage(error);
      this.logger.log({
        level: "error",
        source: "task-orchestrator.follow-up",
        message: "Follow-up prompt failed.",
        context: { taskId: normalizedTaskId },
        error: toStructuredError(error),
      });
      this.transitionTaskToFailed(normalizedTaskId, failureMessage);
    } finally {
      this.runningTaskIds.delete(normalizedTaskId);
    }
  }

  async mergeTask(taskId: string): Promise<MergeTaskWorktreeResult> {
    await this.ensureInitialized();

    const normalizedTaskId = normalizeId(taskId, "Task id");
    const task = this.getTaskOrThrow(normalizedTaskId);

    if (task.state !== "review") {
      throw new Error(`Task ${normalizedTaskId} must be in review state to merge (current: ${task.state}).`);
    }

    if (!task.worktreeDirectory) {
      throw new Error(`Task ${normalizedTaskId} is missing worktree directory.`);
    }

    const project = await this.resolveProject(task.projectId);

    try {
      const mergeResult = await this.worktreeManager.mergeTaskWorktree({
        projectDirectory: project.rootDirectory,
        taskId: normalizedTaskId,
        worktreeDirectory: task.worktreeDirectory,
      });

      const completedRuntime = this.transitionTask(normalizedTaskId, "completed");
      this.emit({
        type: "task.merged",
        taskId: normalizedTaskId,
        branch: mergeResult.branch,
        task: completedRuntime,
      });

      return mergeResult;
    } catch (error) {
      const failureMessage = toErrorMessage(error);
      this.logger.log({
        level: "error",
        source: "task-orchestrator.merge",
        message: "Task merge failed.",
        context: { taskId: normalizedTaskId },
        error: toStructuredError(error),
      });
      this.transitionTaskToFailed(normalizedTaskId, failureMessage);
      throw error;
    }
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

  private async ensureInitialized(): Promise<void> {
    if (this.initialized) {
      return;
    }

    if (!this.initializationPromise) {
      this.initializationPromise = this.loadPersistedTasks()
        .catch((error) => {
          this.logger.log({
            level: "error",
            source: "task-orchestrator.load",
            message: "Failed to load persisted tasks.",
            error: toStructuredError(error),
          });
        })
        .finally(() => {
          this.initialized = true;
          this.initializationPromise = undefined;
        });
    }

    await this.initializationPromise;
  }

  private async loadPersistedTasks(): Promise<void> {
    if (!this.taskRegistry) {
      return;
    }

    const persistedTasks = await this.taskRegistry.listTasks();
    for (const task of persistedTasks) {
      this.tasksById.set(task.taskId, task);
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

      const promptExecution = await this.conversationManager.sendInitialPromptAndAwaitMessages({
        sessionID: createdSession.sessionID,
        prompt: entry.input.initialPrompt,
        worktreeDirectory: createdWorktree.directory,
        model: entry.input.model,
        onMessage: (message) => {
          this.emit({
            type: "task.session.message.received",
            taskId,
            sessionID: createdSession.sessionID,
            message,
          });
        },
      });
      promptSubmission = promptExecution.submission;
      this.emit({
        type: "task.prompt.submitted",
        taskId,
        prompt: promptSubmission,
      });

      runtime = this.transitionTask(taskId, "review");
      this.emit({
        type: "task.review",
        taskId,
        task: runtime,
      });
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

    if ((runtime.state === "review" || runtime.state === "completed") && project && worktree && session && promptSubmission) {
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
    this.persistTask(nextTask);
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
    this.persistTask(nextTask);

    return nextTask;
  }

  private persistTask(task: TaskRuntime): void {
    if (!this.taskRegistry) {
      return;
    }

    void this.taskRegistry.upsertTask(task).catch((error) => {
      this.logger.log({
        level: "error",
        source: "task-orchestrator.persist",
        message: "Failed to persist task.",
        context: {
          taskId: task.taskId,
          state: task.state,
        },
        error: toStructuredError(error),
      });
    });
  }

  private removePersistedTask(taskId: string): void {
    if (!this.taskRegistry) {
      return;
    }

    void this.taskRegistry.removeTask(taskId).catch((error) => {
      this.logger.log({
        level: "error",
        source: "task-orchestrator.persist",
        message: "Failed to remove persisted task.",
        context: {
          taskId,
        },
        error: toStructuredError(error),
      });
    });
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
