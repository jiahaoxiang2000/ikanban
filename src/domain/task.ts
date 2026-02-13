export const TASK_STATES = [
  "queued",
  "creating_worktree",
  "running",
  "review",
  "completed",
  "failed",
  "cleaning",
] as const;

export type TaskState = (typeof TASK_STATES)[number];

export type TaskRuntime = {
  taskId: string;
  projectId: string;
  state: TaskState;
  worktreeDirectory?: string;
  sessionID?: string;
  error?: string;
  model?: {
    providerID: string;
    modelID: string;
  };
  createdAt: number;
  updatedAt: number;
};

export const TASK_STATE_TRANSITIONS: Record<TaskState, readonly TaskState[]> = {
  queued: ["creating_worktree", "failed"],
  creating_worktree: ["running", "failed"],
  running: ["review", "failed", "cleaning"],
  review: ["running", "completed", "failed", "cleaning"],
  completed: ["cleaning"],
  failed: ["cleaning"],
  cleaning: ["completed", "failed"],
};

type TransitionTaskRuntimeOptions = {
  at?: number;
  error?: string;
};

export function canTransitionTaskState(from: TaskState, to: TaskState): boolean {
  return TASK_STATE_TRANSITIONS[from].includes(to);
}

export function assertTaskStateTransition(from: TaskState, to: TaskState): void {
  if (canTransitionTaskState(from, to)) {
    return;
  }

  throw new Error(`Invalid task state transition: ${from} -> ${to}`);
}

export function transitionTaskState(
  task: TaskRuntime,
  to: TaskState,
  options: TransitionTaskRuntimeOptions = {},
): TaskRuntime {
  assertTaskStateTransition(task.state, to);

  const nextTask: TaskRuntime = {
    ...task,
    state: to,
    updatedAt: options.at ?? Date.now(),
    error: resolveTaskError(task, to, options.error),
  };

  assertTaskRuntimeInvariants(nextTask);

  return nextTask;
}

export function validateTaskRuntimeInvariants(task: TaskRuntime): string[] {
  const errors: string[] = [];

  if (task.taskId.trim().length === 0) {
    errors.push("TaskRuntime taskId must be a non-empty string.");
  }

  if (task.projectId.trim().length === 0) {
    errors.push("TaskRuntime projectId must be a non-empty string.");
  }

  if (!Number.isFinite(task.createdAt) || task.createdAt <= 0) {
    errors.push("TaskRuntime createdAt must be a positive timestamp.");
  }

  if (!Number.isFinite(task.updatedAt) || task.updatedAt <= 0) {
    errors.push("TaskRuntime updatedAt must be a positive timestamp.");
  }

  if (task.updatedAt < task.createdAt) {
    errors.push("TaskRuntime updatedAt cannot be earlier than createdAt.");
  }

  if (task.state === "queued") {
    if (task.worktreeDirectory) {
      errors.push("Queued task must not have a worktreeDirectory.");
    }

    if (task.sessionID) {
      errors.push("Queued task must not have a sessionID.");
    }
  }

  if (task.state === "creating_worktree" && task.sessionID) {
    errors.push("Task in creating_worktree state must not have a sessionID.");
  }

  if (
    (task.state === "running" ||
      task.state === "review" ||
      task.state === "completed" ||
      task.state === "cleaning") &&
    !task.worktreeDirectory
  ) {
    errors.push(`Task in ${task.state} state must have a worktreeDirectory.`);
  }

  if ((task.state === "running" || task.state === "review" || task.state === "completed") && !task.sessionID) {
    errors.push(`Task in ${task.state} state must have a sessionID.`);
  }

  if (task.state === "failed" && (!task.error || task.error.trim().length === 0)) {
    errors.push("Task in failed state must include a non-empty error message.");
  }

  return errors;
}

export function assertTaskRuntimeInvariants(task: TaskRuntime): void {
  const errors = validateTaskRuntimeInvariants(task);
  if (errors.length === 0) {
    return;
  }

  throw new Error(`Invalid TaskRuntime: ${errors.join(" ")}`);
}

function resolveTaskError(
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
