import { mkdir } from "node:fs/promises";
import { dirname } from "node:path";

import { assertTaskRuntimeInvariants, type TaskRuntime } from "../domain/task";

const TASK_REGISTRY_STATE_VERSION = 1;

type TaskRegistryState = {
  version: number;
  tasks: TaskRuntime[];
};

export type TaskRegistryOptions = {
  stateFilePath: string;
};

export class TaskRegistry {
  private readonly options: TaskRegistryOptions;
  private readonly tasksById = new Map<string, TaskRuntime>();
  private loadPromise?: Promise<void>;
  private loaded = false;

  constructor(options: TaskRegistryOptions) {
    this.options = options;
  }

  async listTasks(): Promise<TaskRuntime[]> {
    await this.ensureLoaded();
    return this.listTaskSnapshot();
  }

  async upsertTask(task: TaskRuntime): Promise<void> {
    await this.ensureLoaded();
    assertTaskRuntimeInvariants(task);
    this.tasksById.set(task.taskId, task);
    await this.persist();
  }

  async removeTask(taskId: string): Promise<boolean> {
    await this.ensureLoaded();
    const normalizedTaskId = taskId.trim();
    if (!normalizedTaskId) {
      throw new Error("Task id is required.");
    }

    const removed = this.tasksById.delete(normalizedTaskId);
    if (!removed) {
      return false;
    }

    await this.persist();
    return true;
  }

  private listTaskSnapshot(): TaskRuntime[] {
    return [...this.tasksById.values()].sort((left, right) => {
      if (left.createdAt !== right.createdAt) {
        return left.createdAt - right.createdAt;
      }

      return left.taskId.localeCompare(right.taskId);
    });
  }

  private async ensureLoaded(): Promise<void> {
    if (this.loaded) {
      return;
    }

    if (!this.loadPromise) {
      this.loadPromise = this.loadState().finally(() => {
        this.loaded = true;
        this.loadPromise = undefined;
      });
    }

    await this.loadPromise;
  }

  private async loadState(): Promise<void> {
    const stateFile = Bun.file(this.options.stateFilePath);
    const exists = await stateFile.exists();

    if (!exists) {
      return;
    }

    const fileContent = await stateFile.text();
    if (!fileContent.trim()) {
      return;
    }

    const parsedState = this.parseState(fileContent);
    for (const task of parsedState.tasks) {
      this.tasksById.set(task.taskId, task);
    }
  }

  private parseState(fileContent: string): TaskRegistryState {
    const parsedValue = JSON.parse(fileContent) as Partial<TaskRegistryState>;

    if (!parsedValue || typeof parsedValue !== "object") {
      throw new Error("Invalid task registry state: expected an object.");
    }

    if (parsedValue.version !== TASK_REGISTRY_STATE_VERSION) {
      throw new Error(`Unsupported task registry state version: ${parsedValue.version ?? "unknown"}.`);
    }

    if (!Array.isArray(parsedValue.tasks)) {
      throw new Error("Invalid task registry state: tasks must be an array.");
    }

    const tasks = parsedValue.tasks.map((taskLike) => {
      const task: TaskRuntime = {
        taskId: String(taskLike.taskId),
        projectId: String(taskLike.projectId),
        state: String(taskLike.state) as TaskRuntime["state"],
        worktreeDirectory:
          typeof taskLike.worktreeDirectory === "string" ? taskLike.worktreeDirectory : undefined,
        sessionID: typeof taskLike.sessionID === "string" ? taskLike.sessionID : undefined,
        error: typeof taskLike.error === "string" ? taskLike.error : undefined,
        createdAt: Number(taskLike.createdAt),
        updatedAt: Number(taskLike.updatedAt),
      };

      assertTaskRuntimeInvariants(task);
      return task;
    });

    const seenTaskIds = new Set<string>();
    for (const task of tasks) {
      if (seenTaskIds.has(task.taskId)) {
        throw new Error(`Invalid task registry state: duplicate taskId ${task.taskId}.`);
      }

      seenTaskIds.add(task.taskId);
    }

    return {
      version: TASK_REGISTRY_STATE_VERSION,
      tasks,
    };
  }

  private async persist(): Promise<void> {
    await mkdir(dirname(this.options.stateFilePath), { recursive: true });

    const state: TaskRegistryState = {
      version: TASK_REGISTRY_STATE_VERSION,
      tasks: this.listTaskSnapshot(),
    };

    await Bun.write(this.options.stateFilePath, `${JSON.stringify(state, null, 2)}\n`);
  }
}
