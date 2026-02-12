import type { TaskState } from "../domain/task";
import type { WorktreeCleanupPolicy } from "./worktree-manager";

export type RuntimeEventMap = {
  "task.created": {
    taskId: string;
    projectId: string;
    state: TaskState;
    createdAt: number;
  };
  "task.state.updated": {
    taskId: string;
    projectId: string;
    previousState: TaskState;
    nextState: TaskState;
    updatedAt: number;
    error?: string;
  };
  "task.completed": {
    taskId: string;
    projectId: string;
    completedAt: number;
  };
  "task.failed": {
    taskId: string;
    projectId: string;
    failedAt: number;
    error: string;
  };
  "worktree.created": {
    taskId: string;
    projectId: string;
    directory: string;
    branch: string;
    name: string;
    createdAt: number;
  };
  "worktree.removed": {
    taskId: string;
    projectId: string;
    directory: string;
    removedAt: number;
  };
  "worktree.cleanup": {
    taskId: string;
    projectId: string;
    policy: WorktreeCleanupPolicy;
    worktreeDirectory?: string;
    removed: boolean;
    updatedAt: number;
  };
  "session.created": {
    taskId: string;
    projectId: string;
    sessionID: string;
    directory: string;
    createdAt: number;
    title?: string;
  };
  "session.prompt.submitted": {
    taskId: string;
    projectId: string;
    sessionID: string;
    prompt: string;
    submittedAt: number;
  };
  "session.message.received": {
    taskId: string;
    projectId: string;
    sessionID: string;
    messageID: string;
    createdAt: number;
    preview: string;
    role: string;
  };
  "log.appended": {
    level: "info" | "warn" | "error";
    message: string;
    taskId?: string;
    projectId?: string;
    source?: string;
    eventType?: string;
    raw?: unknown;
  };
};

export type RuntimeEventType = keyof RuntimeEventMap;
export type RuntimeLifecycleEventType = Exclude<RuntimeEventType, "log.appended">;

export type RuntimeEventEnvelope<TType extends RuntimeEventType = RuntimeEventType> = {
  type: TType;
  payload: Readonly<RuntimeEventMap[TType]>;
  sequence: number;
  emittedAt: number;
};

export type RuntimeEventListener<TType extends RuntimeEventType = RuntimeEventType> = (
  event: RuntimeEventEnvelope<TType>,
) => void;

export type RuntimeUiUpdate = {
  sequence: number;
  emittedAt: number;
  taskId: string;
  projectId: string;
  scope: "task" | "worktree" | "session";
  action: string;
  eventType: RuntimeLifecycleEventType;
};

export type RuntimeLogEntry = {
  sequence: number;
  emittedAt: number;
  level: "info" | "warn" | "error";
  message: string;
  taskId?: string;
  projectId?: string;
  source: string;
  eventType?: string;
  raw?: unknown;
};

type ListenerDisposer = () => void;

type ListenerRegistration<TListener> = {
  listener: TListener;
  types?: ReadonlySet<RuntimeEventType>;
};

export class RuntimeEventBus {
  private sequence = 0;
  private nextListenerId = 1;
  private readonly listeners = new Map<number, ListenerRegistration<RuntimeEventListener>>();
  private readonly uiListeners = new Map<number, ListenerRegistration<(update: RuntimeUiUpdate) => void>>();
  private readonly logListeners = new Map<number, ListenerRegistration<(entry: RuntimeLogEntry) => void>>();

  emit<TType extends RuntimeEventType>(
    type: TType,
    payload: RuntimeEventMap[TType],
  ): RuntimeEventEnvelope<TType> {
    const event: RuntimeEventEnvelope<TType> = {
      type,
      payload,
      sequence: ++this.sequence,
      emittedAt: Date.now(),
    };

    this.dispatchEvent(event);

    if (isLifecycleEvent(event)) {
      this.dispatchUiUpdate(toUiUpdate(event));
    }

    this.dispatchLogEntry(toLogEntry(event));

    return event;
  }

  subscribe(listener: RuntimeEventListener, options?: { types?: readonly RuntimeEventType[] }): ListenerDisposer {
    return this.register(this.listeners, listener, options?.types);
  }

  subscribeToUiUpdates(
    listener: (update: RuntimeUiUpdate) => void,
    options?: { types?: readonly Exclude<RuntimeEventType, "log.appended">[] },
  ): ListenerDisposer {
    const types = options?.types ? new Set<RuntimeEventType>(options.types) : undefined;
    return this.register(this.uiListeners, listener, types);
  }

  subscribeToLogs(listener: (entry: RuntimeLogEntry) => void): ListenerDisposer {
    return this.register(this.logListeners, listener);
  }

  clear(): void {
    this.listeners.clear();
    this.uiListeners.clear();
    this.logListeners.clear();
  }

  listenerCount(): number {
    return this.listeners.size + this.uiListeners.size + this.logListeners.size;
  }

  private dispatchEvent(event: RuntimeEventEnvelope): void {
    const registrations = Array.from(this.listeners.values());

    for (const registration of registrations) {
      if (registration.types && !registration.types.has(event.type)) {
        continue;
      }

      registration.listener(event);
    }
  }

  private dispatchUiUpdate(update: RuntimeUiUpdate): void {
    const registrations = Array.from(this.uiListeners.values());

    for (const registration of registrations) {
      if (registration.types && !registration.types.has(update.eventType)) {
        continue;
      }

      registration.listener(update);
    }
  }

  private dispatchLogEntry(entry: RuntimeLogEntry): void {
    const registrations = Array.from(this.logListeners.values());

    for (const registration of registrations) {
      registration.listener(entry);
    }
  }

  private register<TListener>(
    target: Map<number, ListenerRegistration<TListener>>,
    listener: TListener,
    types?: Iterable<RuntimeEventType>,
  ): ListenerDisposer {
    const id = this.nextListenerId++;
    target.set(id, {
      listener,
      types: types ? new Set(types) : undefined,
    });

    let active = true;
    return () => {
      if (!active) {
        return;
      }

      active = false;
      target.delete(id);
    };
  }
}

function toUiUpdate(event: RuntimeEventEnvelope<Exclude<RuntimeEventType, "log.appended">>): RuntimeUiUpdate {
  const payload = event.payload as { taskId: string; projectId: string };
  const [scope, action] = event.type.split(".") as ["task" | "worktree" | "session", string];

  return {
    sequence: event.sequence,
    emittedAt: event.emittedAt,
    taskId: payload.taskId,
    projectId: payload.projectId,
    scope,
    action,
    eventType: event.type,
  };
}

function toLogEntry(event: RuntimeEventEnvelope): RuntimeLogEntry {
  if (isLogEvent(event)) {
    const payload = event.payload;
    return {
      sequence: event.sequence,
      emittedAt: event.emittedAt,
      level: payload.level,
      message: payload.message,
      taskId: payload.taskId,
      projectId: payload.projectId,
      source: payload.source ?? "runtime",
      eventType: payload.eventType,
      raw: payload.raw,
    };
  }

  const lifecycleEvent = event as RuntimeEventEnvelope<RuntimeLifecycleEventType>;

  return {
    sequence: lifecycleEvent.sequence,
    emittedAt: lifecycleEvent.emittedAt,
    level: lifecycleEvent.type === "task.failed" ? "error" : "info",
    message: toDefaultLogMessage(lifecycleEvent),
    taskId: (lifecycleEvent.payload as { taskId?: string }).taskId,
    projectId: (lifecycleEvent.payload as { projectId?: string }).projectId,
    source: "event-bus",
    eventType: lifecycleEvent.type,
    raw: lifecycleEvent.payload,
  };
}

function toDefaultLogMessage(event: RuntimeEventEnvelope<Exclude<RuntimeEventType, "log.appended">>): string {
  const payload = event.payload as Record<string, unknown>;

  switch (event.type) {
    case "task.created":
      return `Task ${String(payload.taskId)} created in state ${String(payload.state)}.`;
    case "task.state.updated":
      return `Task ${String(payload.taskId)} transitioned ${String(payload.previousState)} -> ${String(payload.nextState)}.`;
    case "task.completed":
      return `Task ${String(payload.taskId)} completed.`;
    case "task.failed":
      return `Task ${String(payload.taskId)} failed: ${String(payload.error)}.`;
    case "worktree.created":
      return `Worktree ${String(payload.name)} created at ${String(payload.directory)}.`;
    case "worktree.removed":
      return `Worktree removed at ${String(payload.directory)}.`;
    case "worktree.cleanup":
      return `Worktree cleanup (${String(payload.policy)}) removed=${String(payload.removed)}.`;
    case "session.created":
      return `Session ${String(payload.sessionID)} created.`;
    case "session.prompt.submitted":
      return `Prompt submitted to session ${String(payload.sessionID)}.`;
    case "session.message.received":
      return `Message ${String(payload.messageID)} received for session ${String(payload.sessionID)}.`;
  }
}

function isLifecycleEvent(
  event: RuntimeEventEnvelope,
): event is RuntimeEventEnvelope<RuntimeLifecycleEventType> {
  return event.type !== "log.appended";
}

function isLogEvent(event: RuntimeEventEnvelope): event is RuntimeEventEnvelope<"log.appended"> {
  return event.type === "log.appended";
}
