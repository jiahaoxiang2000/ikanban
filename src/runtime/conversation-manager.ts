import { resolve } from "node:path";

import {
  toConversationMessageMeta,
  type ConversationMessageLike,
  type ConversationMessageMeta,
  type ConversationSessionMeta,
} from "../domain/conversation";
import type { OpenCodeRuntime } from "./opencode-runtime";

type RuntimeClientProvider = Pick<OpenCodeRuntime, "getClient">;

type ConversationApiResponse<TData> = {
  data?: TData;
  error?: unknown;
};

type CreateSessionPayload = {
  id?: string;
  sessionID?: string;
  title?: string;
  createdAt?: number;
  updatedAt?: number;
};

export type CreateConversationSessionInput = {
  projectId: string;
  taskId: string;
  projectDirectory: string;
  worktreeDirectory: string;
  title?: string;
  timestamp?: number;
};

export type SendInitialPromptInput = {
  sessionID: string;
  prompt: string;
  worktreeDirectory?: string;
  agent?: string;
  model?: {
    providerID: string;
    modelID: string;
  };
};

export type SendFollowUpPromptInput = {
  sessionID: string;
  prompt: string;
  worktreeDirectory?: string;
  agent?: string;
  model?: {
    providerID: string;
    modelID: string;
  };
};

export type PromptSubmission = {
  sessionID: string;
  prompt: string;
  submittedAt: number;
};

export type PromptExecutionResult = {
  submission: PromptSubmission;
  messages: ConversationMessageMeta[];
};

type PromptMessageHandler = (message: ConversationMessageMeta) => void;

export type ListConversationMessagesInput = {
  sessionID: string;
  worktreeDirectory?: string;
};

export type SubscribeToConversationEventsInput = {
  sessionID?: string;
  worktreeDirectory?: string;
  onEvent?: (event: unknown) => void;
};

export type ConversationEventSubscription = {
  directory: string;
  unsubscribe: () => Promise<void>;
};

type EventSubscribeHandle = {
  unsubscribe?: () => void | Promise<void>;
  [Symbol.asyncIterator]?: () => AsyncIterator<unknown>;
};

export class ConversationManager {
  private readonly runtime: RuntimeClientProvider;
  private readonly taskToSessionID = new Map<string, string>();
  private readonly sessionToDirectory = new Map<string, string>();
  private readonly sessionsByID = new Map<string, ConversationSessionMeta>();

  constructor(runtime: RuntimeClientProvider) {
    this.runtime = runtime;
  }

  async createTaskSession(input: CreateConversationSessionInput): Promise<ConversationSessionMeta> {
    const projectId = normalizeId(input.projectId, "Project id");
    const taskId = normalizeId(input.taskId, "Task id");
    const projectDirectory = normalizeDirectory(input.projectDirectory, "Project directory");
    const worktreeDirectory = normalizeDirectory(input.worktreeDirectory, "Worktree directory");
    const title = normalizeOptionalTitle(input.title);
    const fallbackTimestamp = normalizeTimestamp(input.timestamp ?? Date.now(), "Timestamp");
    const client = await this.runtime.getClient(worktreeDirectory);
    const payload = await readDataOrThrow<CreateSessionPayload>(
      client.session.create({
        directory: worktreeDirectory,
        title,
      }),
      "Failed to create conversation session",
    );
    const sessionID = normalizeSessionID(payload.sessionID ?? payload.id);
    const createdAt = normalizeOptionalTimestamp(payload.createdAt, fallbackTimestamp);
    const updatedAt = normalizeOptionalTimestamp(payload.updatedAt, createdAt);

    const session: ConversationSessionMeta = {
      sessionID,
      projectId,
      taskId,
      directory: worktreeDirectory,
      title: payload.title ?? title,
      createdAt,
      updatedAt,
    };

    this.taskToSessionID.set(taskId, sessionID);
    this.sessionToDirectory.set(sessionID, worktreeDirectory);
    this.sessionsByID.set(sessionID, session);

    return session;
  }

  async sendInitialPrompt(input: SendInitialPromptInput): Promise<PromptSubmission> {
    return this.sendPrompt(input, "Failed to send initial prompt");
  }

  async sendInitialPromptAndAwaitMessages(
    input: SendInitialPromptInput & { timeoutMs?: number; onMessage?: PromptMessageHandler },
  ): Promise<PromptExecutionResult> {
    const sessionID = normalizeSessionID(input.sessionID);
    const prompt = normalizePrompt(input.prompt);
    const worktreeDirectory = this.resolveDirectoryForSession(sessionID, input.worktreeDirectory);
    const client = await this.runtime.getClient(worktreeDirectory);
    const timeoutMs = normalizeOptionalTimeout(input.timeoutMs, 45_000);
    const resolvedModel = await this.resolvePromptModel(client, worktreeDirectory, input.model);
    const resolvedAgent = input.agent?.trim() || "build";
    const onMessage = input.onMessage;
    const existingMessages = await this.listConversationMessages({
      sessionID,
      worktreeDirectory,
    });
    const knownMessageStates = new Map(
      existingMessages.map((message) => [message.id, toMessageStateSignature(message)]),
    );
    const relevantMessages: ConversationMessageMeta[] = [];
    let lastPollAt = 0;

    const pollForNewMessages = async (force = false): Promise<void> => {
      const now = Date.now();
      if (!force && now - lastPollAt < 750) {
        return;
      }

      lastPollAt = now;
      const newlyObserved = await this.collectMessageChanges({
        sessionID,
        worktreeDirectory,
        knownMessageStates,
      });

      for (const message of newlyObserved) {
        relevantMessages.push(message);
        onMessage?.(message);
      }
    };

    const subscribeResult = await client.event.subscribe({
      directory: worktreeDirectory,
    });
    const eventStream = extractEventStream(subscribeResult);
    const iterator = eventStream[Symbol.asyncIterator]();

    const submittedAt = Date.now();

    const promptResponse = await client.session.promptAsync({
      sessionID,
      parts: [{ type: "text", text: prompt }],
      agent: resolvedAgent,
      model: resolvedModel,
    });

    if (promptResponse.error) {
      throw new Error(`Failed to send initial prompt: ${formatUnknownError(promptResponse.error)}`);
    }

    const existing = this.sessionsByID.get(sessionID);
    if (existing) {
      this.sessionsByID.set(sessionID, {
        ...existing,
        updatedAt: submittedAt,
        lastMessageAt: submittedAt,
      });
    }

    try {
      const idleResult = await waitForSessionIdle(iterator, sessionID, timeoutMs, {
        onSessionEvent: async (event) => {
          if (isMessageStreamEvent(event.type)) {
            await pollForNewMessages(true);
          }
        },
        onTick: async () => {
          await pollForNewMessages(false);
        },
      });
      if (idleResult.errorMessage) {
        throw new Error(idleResult.errorMessage);
      }

      await pollForNewMessages(true);

      if (!idleResult.idle && relevantMessages.length === 0) {
        throw new Error(`No assistant response received within ${timeoutMs}ms for session ${sessionID}.`);
      }
    } finally {
      await iterator.return?.();
    }

    await pollForNewMessages(true);
    const hasAssistantMessage = relevantMessages.some((message) => message.role === "assistant");

    if (!hasAssistantMessage) {
      throw new Error(`No assistant response received within ${timeoutMs}ms for session ${sessionID}.`);
    }

    return {
      submission: {
        sessionID,
        prompt,
        submittedAt,
      },
      messages: relevantMessages,
    };
  }

  async sendFollowUpPrompt(input: SendFollowUpPromptInput): Promise<PromptSubmission> {
    return this.sendPrompt(input, "Failed to send follow-up prompt");
  }

  async sendFollowUpPromptAndAwaitMessages(
    input: SendFollowUpPromptInput & { timeoutMs?: number; onMessage?: PromptMessageHandler },
  ): Promise<PromptExecutionResult> {
    const sessionID = normalizeSessionID(input.sessionID);
    const prompt = normalizePrompt(input.prompt);
    const worktreeDirectory = this.resolveDirectoryForSession(sessionID, input.worktreeDirectory);
    const client = await this.runtime.getClient(worktreeDirectory);
    const timeoutMs = normalizeOptionalTimeout(input.timeoutMs, 45_000);
    const resolvedModel = await this.resolvePromptModel(client, worktreeDirectory, input.model);
    const resolvedAgent = input.agent?.trim() || "build";
    const onMessage = input.onMessage;
    const existingMessages = await this.listConversationMessages({
      sessionID,
      worktreeDirectory,
    });
    const knownMessageStates = new Map(
      existingMessages.map((message) => [message.id, toMessageStateSignature(message)]),
    );
    const relevantMessages: ConversationMessageMeta[] = [];
    let lastPollAt = 0;

    const pollForNewMessages = async (force = false): Promise<void> => {
      const now = Date.now();
      if (!force && now - lastPollAt < 750) {
        return;
      }

      lastPollAt = now;
      const newlyObserved = await this.collectMessageChanges({
        sessionID,
        worktreeDirectory,
        knownMessageStates,
      });

      for (const message of newlyObserved) {
        relevantMessages.push(message);
        onMessage?.(message);
      }
    };

    const subscribeResult = await client.event.subscribe({
      directory: worktreeDirectory,
    });
    const eventStream = extractEventStream(subscribeResult);
    const iterator = eventStream[Symbol.asyncIterator]();

    const submittedAt = Date.now();

    const promptResponse = await client.session.promptAsync({
      sessionID,
      parts: [{ type: "text", text: prompt }],
      agent: resolvedAgent,
      model: resolvedModel,
    });

    if (promptResponse.error) {
      throw new Error(`Failed to send follow-up prompt: ${formatUnknownError(promptResponse.error)}`);
    }

    const existing = this.sessionsByID.get(sessionID);
    if (existing) {
      this.sessionsByID.set(sessionID, {
        ...existing,
        updatedAt: submittedAt,
        lastMessageAt: submittedAt,
      });
    }

    try {
      const idleResult = await waitForSessionIdle(iterator, sessionID, timeoutMs, {
        onSessionEvent: async (event) => {
          if (isMessageStreamEvent(event.type)) {
            await pollForNewMessages(true);
          }
        },
        onTick: async () => {
          await pollForNewMessages(false);
        },
      });
      if (idleResult.errorMessage) {
        throw new Error(idleResult.errorMessage);
      }

      await pollForNewMessages(true);

      if (!idleResult.idle && relevantMessages.length === 0) {
        throw new Error(`No assistant response received within ${timeoutMs}ms for session ${sessionID}.`);
      }
    } finally {
      await iterator.return?.();
    }

    await pollForNewMessages(true);
    const hasAssistantMessage = relevantMessages.some((message) => message.role === "assistant");

    if (!hasAssistantMessage) {
      throw new Error(`No assistant response received within ${timeoutMs}ms for session ${sessionID}.`);
    }

    return {
      submission: {
        sessionID,
        prompt,
        submittedAt,
      },
      messages: relevantMessages,
    };
  }

  async listConversationMessages(
    input: ListConversationMessagesInput,
  ): Promise<ConversationMessageMeta[]> {
    const sessionID = normalizeSessionID(input.sessionID);
    const worktreeDirectory = this.resolveDirectoryForSession(sessionID, input.worktreeDirectory);
    const client = await this.runtime.getClient(worktreeDirectory);
    const messages = await readDataOrThrow<unknown[]>(
      client.session.messages({
        sessionID,
      }),
      "Failed to list conversation messages",
    );

    const normalizedMessages = messages.map((message, index) =>
      normalizeMessageLike(message, sessionID, index),
    );

    return normalizedMessages.map((message) => toConversationMessageMeta(message));
  }

  async subscribeToEvents(
    input: SubscribeToConversationEventsInput,
  ): Promise<ConversationEventSubscription> {
    const sessionID = input.sessionID ? normalizeSessionID(input.sessionID) : undefined;
    const worktreeDirectory = sessionID
      ? this.resolveDirectoryForSession(sessionID, input.worktreeDirectory)
      : normalizeDirectoryOrThrow(input.worktreeDirectory, "Worktree directory");
    const client = await this.runtime.getClient(worktreeDirectory);
    const subscribeResult = await client.event.subscribe({
      directory: worktreeDirectory,
    });
    const subscribePayload = unwrapResponseDataOrThrow(
      subscribeResult,
      "Failed to subscribe to conversation events",
    );
    const unsubscribe = toAsyncUnsubscribe(subscribePayload, input.onEvent);

    return {
      directory: worktreeDirectory,
      unsubscribe,
    };
  }

  getTaskSessionID(taskId: string): string | undefined {
    return this.taskToSessionID.get(normalizeId(taskId, "Task id"));
  }

  getSessionDirectory(sessionID: string): string | undefined {
    return this.sessionToDirectory.get(normalizeSessionID(sessionID));
  }

  getSession(sessionID: string): ConversationSessionMeta | undefined {
    return this.sessionsByID.get(normalizeSessionID(sessionID));
  }

  private async sendPrompt(
    input: SendInitialPromptInput | SendFollowUpPromptInput,
    failureMessage: string,
  ): Promise<PromptSubmission> {
    const sessionID = normalizeSessionID(input.sessionID);
    const prompt = normalizePrompt(input.prompt);
    const worktreeDirectory = this.resolveDirectoryForSession(sessionID, input.worktreeDirectory);
    const client = await this.runtime.getClient(worktreeDirectory);
    const resolvedModel = await this.resolvePromptModel(client, worktreeDirectory, input.model);
    const resolvedAgent = input.agent?.trim() || "build";

    await readDataOrThrow<unknown>(
      client.session.prompt({
        sessionID,
        parts: [{ type: "text", text: prompt }],
        agent: resolvedAgent,
        model: resolvedModel,
      }),
      failureMessage,
    );

    const submittedAt = Date.now();
    const existing = this.sessionsByID.get(sessionID);

    if (existing) {
      this.sessionsByID.set(sessionID, {
        ...existing,
        updatedAt: submittedAt,
        lastMessageAt: submittedAt,
      });
    }

    return {
      sessionID,
      prompt,
      submittedAt,
    };
  }

  private async collectMessageChanges(input: {
    sessionID: string;
    worktreeDirectory: string;
    knownMessageStates: Map<string, string>;
  }): Promise<ConversationMessageMeta[]> {
    const messages = await this.listConversationMessages({
      sessionID: input.sessionID,
      worktreeDirectory: input.worktreeDirectory,
    });

    const changes: ConversationMessageMeta[] = [];
    for (const message of messages) {
      const signature = toMessageStateSignature(message);
      const previousSignature = input.knownMessageStates.get(message.id);

      if (previousSignature === signature) {
        continue;
      }

      input.knownMessageStates.set(message.id, signature);
      changes.push(message);
    }

    return changes;
  }

  private resolveDirectoryForSession(sessionID: string, explicitDirectory?: string): string {
    if (explicitDirectory) {
      const normalizedDirectory = normalizeDirectory(explicitDirectory, "Worktree directory");
      this.sessionToDirectory.set(sessionID, normalizedDirectory);
      return normalizedDirectory;
    }

    const mappedDirectory = this.sessionToDirectory.get(sessionID);
    if (mappedDirectory) {
      return mappedDirectory;
    }

    throw new Error(`Worktree directory is required for session ${sessionID}.`);
  }

  private async resolvePromptModel(
    client: Awaited<ReturnType<RuntimeClientProvider["getClient"]>>,
    directory: string,
    explicitModel: SendInitialPromptInput["model"] | undefined,
  ): Promise<SendInitialPromptInput["model"] | undefined> {
    if (explicitModel) {
      return explicitModel;
    }

    const providersResponse = await readDataOrThrow<
      {
        providers?: Array<{
          id?: string;
          models?: Record<string, unknown>;
        }>;
        default?: Record<string, string>;
      }
    >(
      client.config.providers({ directory }),
      "Failed to resolve default model",
    );

    const providers = providersResponse.providers ?? [];
    const defaultByProvider = providersResponse.default ?? {};
    const preferredProviders = ["openai", "anthropic", "google", "opencode"];
    const preferredOrder = [
      ...preferredProviders,
      ...Object.keys(defaultByProvider).filter((providerID) => !preferredProviders.includes(providerID)),
    ];

    for (const providerID of preferredOrder) {
      const modelID = defaultByProvider[providerID];
      if (!modelID) {
        continue;
      }

      const provider = providers.find((candidate) => candidate.id === providerID);
      if (!provider?.models || !(modelID in provider.models)) {
        continue;
      }

      return {
        providerID,
        modelID,
      };
    }

    return undefined;
  }
}

function normalizeMessageLike(raw: unknown, sessionID: string, index: number): ConversationMessageLike {
  const message = asRecord(raw);
  const info = asRecord(message?.info);
  const time = asRecord(info?.time);

  const id =
    typeof info?.id === "string" && info.id.trim()
      ? info.id
      : typeof message?.id === "string" && message.id.trim()
        ? message.id
        : `${sessionID}:${index}`;
  const role =
    typeof info?.role === "string"
      ? info.role
      : typeof message?.role === "string"
        ? message.role
        : "assistant";
  const createdAt = normalizeOptionalTimestamp(time?.created ?? message?.createdAt, Date.now());
  const parts = normalizeMessageParts(message?.parts);

  return {
    id,
    sessionID,
    role,
    createdAt,
    parts,
    error: message?.error,
  };
}

function normalizeMessageParts(value: unknown): Array<{ text?: string }> {
  if (!Array.isArray(value)) {
    return [];
  }

  const parts: Array<{ text?: string }> = [];

  for (const part of value) {
    if (typeof part === "string") {
      parts.push({ text: part });
      continue;
    }

    const partRecord = asRecord(part);
    if (!partRecord) {
      parts.push({});
      continue;
    }

    if (typeof partRecord.text === "string") {
      parts.push({ text: partRecord.text });
      continue;
    }

    if (typeof partRecord.content === "string") {
      parts.push({ text: partRecord.content });
      continue;
    }

    parts.push({});
  }

  return parts;
}

function toAsyncUnsubscribe(payload: unknown, onEvent?: (event: unknown) => void): () => Promise<void> {
  if (typeof payload === "function") {
    const unsubscribe = payload as () => void | Promise<void>;
    return async () => {
      await unsubscribe();
    };
  }

  const handle = asRecord(payload) as EventSubscribeHandle | undefined;

  if (handle?.unsubscribe && typeof handle.unsubscribe === "function") {
    return async () => {
      await handle.unsubscribe?.();
    };
  }

  const iteratorFactory = handle?.[Symbol.asyncIterator];

  if (typeof iteratorFactory === "function" && onEvent) {
    let closed = false;
    const iterator = iteratorFactory.call(handle);
    const pump = (async () => {
      while (!closed) {
        const nextValue = await iterator.next();
        if (nextValue.done) {
          break;
        }

        onEvent(nextValue.value);
      }
    })();

    return async () => {
      closed = true;
      await iterator.return?.();
      await pump;
    };
  }

  return async () => {};
}

function unwrapResponseDataOrThrow<TData>(response: unknown, failureMessage: string): TData {
  const responseRecord = asRecord(response);

  if (!responseRecord) {
    return response as TData;
  }

  if ("error" in responseRecord && responseRecord.error) {
    throw new Error(`${failureMessage}: ${formatUnknownError(responseRecord.error)}`);
  }

  if ("data" in responseRecord) {
    const data = responseRecord.data;
    if (data === undefined) {
      throw new Error(`${failureMessage}: response did not include data.`);
    }

    return data as TData;
  }

  return response as TData;
}

async function readDataOrThrow<TData>(
  request: Promise<ConversationApiResponse<TData>>,
  failureMessage: string,
): Promise<TData> {
  const response = await request;

  if (response.error) {
    throw new Error(`${failureMessage}: ${formatUnknownError(response.error)}`);
  }

  if (response.data === undefined) {
    throw new Error(`${failureMessage}: response did not include data.`);
  }

  return response.data;
}

function normalizeId(value: string, label: string): string {
  const normalized = value.trim();

  if (!normalized) {
    throw new Error(`${label} is required.`);
  }

  return normalized;
}

function normalizeSessionID(sessionID: string | undefined): string {
  if (!sessionID) {
    throw new Error("Session id is required.");
  }

  return normalizeId(sessionID, "Session id");
}

function normalizePrompt(prompt: string): string {
  const normalized = prompt.trim();

  if (!normalized) {
    throw new Error("Prompt is required.");
  }

  return normalized;
}

function normalizeDirectory(directory: string, label: string): string {
  const normalizedDirectory = directory.trim();

  if (!normalizedDirectory) {
    throw new Error(`${label} is required.`);
  }

  return resolve(normalizedDirectory);
}

function normalizeDirectoryOrThrow(directory: string | undefined, label: string): string {
  if (directory === undefined) {
    throw new Error(`${label} is required.`);
  }

  return normalizeDirectory(directory, label);
}

function normalizeOptionalTitle(title: string | undefined): string | undefined {
  if (!title) {
    return undefined;
  }

  const normalized = title.trim();
  return normalized.length > 0 ? normalized : undefined;
}

function normalizeTimestamp(timestamp: number, label: string): number {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    throw new Error(`${label} must be a positive finite number.`);
  }

  return Math.floor(timestamp);
}

function normalizeOptionalTimestamp(value: unknown, fallback: number): number {
  if (typeof value !== "number") {
    return fallback;
  }

  return normalizeTimestamp(value, "Timestamp");
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }

  return value as Record<string, unknown>;
}

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Unknown SDK error";
}

function normalizeOptionalTimeout(timeoutMs: number | undefined, fallback: number): number {
  if (timeoutMs === undefined) {
    return fallback;
  }

  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new Error("Timeout must be a positive finite number.");
  }

  return Math.floor(timeoutMs);
}

function extractEventStream(subscribeResult: unknown): AsyncIterable<unknown> {
  const resultRecord = asRecord(subscribeResult);
  const streamCandidate = resultRecord?.stream;

  if (
    streamCandidate &&
    typeof streamCandidate === "object" &&
    typeof (streamCandidate as AsyncIterable<unknown>)[Symbol.asyncIterator] === "function"
  ) {
    return streamCandidate as AsyncIterable<unknown>;
  }

  throw new Error("Failed to subscribe to conversation events: missing async event stream.");
}

async function waitForSessionIdle(
  iterator: AsyncIterator<unknown>,
  sessionID: string,
  timeoutMs: number,
  hooks?: {
    onSessionEvent?: (event: { type: string; properties?: unknown }) => Promise<void>;
    onTick?: () => Promise<void>;
  },
): Promise<{ idle: boolean; errorMessage?: string }> {
  const deadline = Date.now() + timeoutMs;
  let nextDeadline = deadline;

  while (Date.now() < nextDeadline) {
    const remainingMs = Math.max(1, nextDeadline - Date.now());
    const next = await nextEventWithTimeout(iterator, Math.min(1_000, remainingMs));

    if (!next) {
      await hooks?.onTick?.();
      continue;
    }

    if (next.done) {
      return { idle: false };
    }

    const normalizedEvent = normalizeEvent(next.value);
    if (!normalizedEvent) {
      continue;
    }

    const eventSessionID = extractEventSessionID(normalizedEvent);
    if (eventSessionID !== sessionID) {
      continue;
    }

    const properties = asRecord(normalizedEvent.properties);
    await hooks?.onSessionEvent?.(normalizedEvent);
    nextDeadline = Date.now() + timeoutMs;

    if (normalizedEvent.type === "session.idle") {
      return { idle: true };
    }

    if (normalizedEvent.type === "session.status") {
      const status = asRecord(properties?.status);
      if (status?.type === "idle") {
        return { idle: true };
      }
      continue;
    }

    if (normalizedEvent.type === "session.error") {
      const errorLike = asRecord(properties?.error);
      const data = asRecord(errorLike?.data);
      const message =
        (typeof data?.message === "string" && data.message) ||
        (typeof errorLike?.name === "string" && errorLike.name) ||
        "Session execution failed.";
      return { idle: false, errorMessage: message };
    }
  }

  await hooks?.onTick?.();

  return { idle: false };
}

function isMessageStreamEvent(type: string): boolean {
  return (
    type === "message.updated" ||
    type === "message.part.updated" ||
    type === "message.part.removed" ||
    type === "message.removed"
  );
}

function toMessageStateSignature(message: ConversationMessageMeta): string {
  return JSON.stringify({
    role: message.role,
    createdAt: message.createdAt,
    preview: message.preview,
    partCount: message.partCount,
    hasError: message.hasError,
  });
}

function extractEventSessionID(event: { type: string; properties?: unknown }): string | undefined {
  const properties = asRecord(event.properties);
  if (!properties) {
    return undefined;
  }

  if (typeof properties.sessionID === "string") {
    return properties.sessionID;
  }

  if (event.type === "message.updated") {
    const info = asRecord(properties.info);
    if (typeof info?.sessionID === "string") {
      return info.sessionID;
    }
  }

  if (event.type === "message.part.updated") {
    const part = asRecord(properties.part);
    if (typeof part?.sessionID === "string") {
      return part.sessionID;
    }
  }

  return undefined;
}

function normalizeEvent(value: unknown): { type: string; properties?: unknown } | undefined {
  const direct = asRecord(value);
  if (!direct) {
    return undefined;
  }

  if (typeof direct.type === "string") {
    return {
      type: direct.type,
      properties: direct.properties,
    };
  }

  const payload = asRecord(direct.payload);
  if (!payload || typeof payload.type !== "string") {
    return undefined;
  }

  return {
    type: payload.type,
    properties: payload.properties,
  };
}

async function nextEventWithTimeout(
  iterator: AsyncIterator<unknown>,
  timeoutMs: number,
): Promise<IteratorResult<unknown> | undefined> {
  const timeoutValue = Symbol("timeout");
  const result = await Promise.race([
    iterator.next(),
    sleep(timeoutMs).then(() => timeoutValue),
  ]);

  if (result === timeoutValue) {
    return undefined;
  }

  return result as IteratorResult<unknown>;
}

function sleep(durationMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, durationMs);
  });
}
