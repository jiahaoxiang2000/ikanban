import { useState, useEffect, useCallback, useRef } from "react"
import type {
  OpencodeClient,
  Session,
  SessionStatus,
  Message,
  Part,
  Permission,
  Todo,
  Event,
  FileDiff,
} from "@opencode-ai/sdk"

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SessionMessage {
  info: Message
  parts: Part[]
}

export interface SessionState {
  /** The session metadata (title, id, timestamps, etc.) */
  session: Session | null
  /** Ordered list of messages + their parts */
  messages: SessionMessage[]
  /** Current session status: idle / busy / retry */
  status: SessionStatus
  /** Pending permission request (null when none) */
  pendingPermission: Permission | null
  /** Agent todo list for this session */
  todos: Todo[]
  /** Files edited during this session */
  editedFiles: string[]
  /** Session diffs (populated via SSE or explicit fetch) */
  diffs: FileDiff[]
  /** Last error from the session */
  error: Event | null
  /** Whether the initial data has been loaded */
  ready: boolean
}

export interface UseSessionActions {
  /** Send a text prompt to the session */
  sendMessage: (text: string) => Promise<void>
  /** Abort the running session */
  abort: () => Promise<void>
  /** Reply to a pending permission request */
  replyPermission: (
    permissionId: string,
    response: "once" | "always" | "reject",
  ) => Promise<void>
}

export interface UseSessionResult {
  state: SessionState
  actions: UseSessionActions
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

const INITIAL_STATUS: SessionStatus = { type: "idle" }

function initialState(): SessionState {
  return {
    session: null,
    messages: [],
    status: INITIAL_STATUS,
    pendingPermission: null,
    todos: [],
    editedFiles: [],
    diffs: [],
    error: null,
    ready: false,
  }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Wraps `client.session.*` + `client.event.subscribe()` to provide real-time
 * session state for a single opencode session.
 *
 * @param client  – An `OpencodeClient` instance (from `createOpencode()`)
 * @param sessionId – The session to observe
 * @param directory – The worktree directory for this agent
 */
export function useSession(
  client: OpencodeClient | null,
  sessionId: string | null,
  directory?: string,
): UseSessionResult {
  const [state, setState] = useState<SessionState>(initialState)

  // Keep a ref so event handlers always see the latest state without
  // re-subscribing on every render.
  const stateRef = useRef(state)
  stateRef.current = state

  // -----------------------------------------------------------------------
  // Helpers to merge state
  // -----------------------------------------------------------------------

  const patch = useCallback((partial: Partial<SessionState>) => {
    setState((prev) => ({ ...prev, ...partial }))
  }, [])

  const upsertMessage = useCallback(
    (info: Message) => {
      setState((prev) => {
        const idx = prev.messages.findIndex((m) => m.info.id === info.id)
        if (idx === -1) {
          return { ...prev, messages: [...prev.messages, { info, parts: [] }] }
        }
        const updated = [...prev.messages]
        const existing = updated[idx]!
        updated[idx] = { info, parts: existing.parts }
        return { ...prev, messages: updated }
      })
    },
    [],
  )

  const upsertPart = useCallback(
    (part: Part) => {
      setState((prev) => {
        const msgIdx = prev.messages.findIndex(
          (m) => m.info.id === part.messageID,
        )
        if (msgIdx === -1) {
          // Message not yet known – buffer the part under a placeholder
          return {
            ...prev,
            messages: [
              ...prev.messages,
              {
                info: {
                  id: part.messageID,
                  sessionID: part.sessionID,
                  role: "assistant",
                  time: { created: Date.now() },
                  parentID: "",
                  modelID: "",
                  providerID: "",
                  mode: "",
                  path: { cwd: "", root: "" },
                  cost: 0,
                  tokens: {
                    input: 0,
                    output: 0,
                    reasoning: 0,
                    cache: { read: 0, write: 0 },
                  },
                } as Message,
                parts: [part],
              },
            ],
          }
        }
        const updated = [...prev.messages]
        const msg = updated[msgIdx]!
        const partIdx = msg.parts.findIndex((p) => p.id === part.id)
        if (partIdx === -1) {
          updated[msgIdx] = { info: msg.info, parts: [...msg.parts, part] }
        } else {
          const updatedParts = [...msg.parts]
          updatedParts[partIdx] = part
          updated[msgIdx] = { info: msg.info, parts: updatedParts }
        }
        return { ...prev, messages: updated }
      })
    },
    [],
  )

  // -----------------------------------------------------------------------
  // Bootstrap: load session + existing messages
  // -----------------------------------------------------------------------

  useEffect(() => {
    if (!client || !sessionId) {
      setState(initialState())
      return
    }

    let cancelled = false
    const q = directory ? { directory } : undefined

    async function load() {
      try {
        const [sessionRes, messagesRes, todosRes] = await Promise.all([
          client!.session.get({ path: { id: sessionId! }, query: q }),
          client!.session.messages({ path: { id: sessionId! }, query: q }),
          client!.session.todo({ path: { id: sessionId! }, query: q }),
        ])

        if (cancelled) return

        const messages: SessionMessage[] = (messagesRes.data ?? []).map(
          (m) => ({
            info: m.info,
            parts: m.parts,
          }),
        )

        patch({
          session: sessionRes.data ?? null,
          messages,
          todos: todosRes.data ?? [],
          ready: true,
        })
      } catch {
        if (!cancelled) {
          patch({ ready: true })
        }
      }
    }

    load()
    return () => {
      cancelled = true
    }
  }, [client, sessionId, directory, patch])

  // -----------------------------------------------------------------------
  // SSE subscription
  // -----------------------------------------------------------------------

  useEffect(() => {
    if (!client || !sessionId) return

    let cancelled = false
    const q = directory ? { directory } : undefined

    async function subscribe() {
      const result = await client!.event.subscribe({ query: q })
      const stream = result.stream

      try {
        for await (const event of stream) {
          if (cancelled) break
          handleEvent(event as Event)
        }
      } catch {
        // stream closed or network error – ignore if unmounted
      }
    }

    function handleEvent(evt: Event) {
      // Only process events relevant to our session
      const sid = sessionId!

      switch (evt.type) {
        // --- Session status (idle / busy / retry) ---
        case "session.status": {
          if (evt.properties.sessionID === sid) {
            patch({ status: evt.properties.status })
          }
          break
        }

        // --- Message updated (new or changed message metadata) ---
        case "message.updated": {
          if (evt.properties.info.sessionID === sid) {
            upsertMessage(evt.properties.info)
          }
          break
        }

        // --- Message part updated (streaming text, tool calls) ---
        case "message.part.updated": {
          if (evt.properties.part.sessionID === sid) {
            upsertPart(evt.properties.part)
          }
          break
        }

        // --- Permission requested ---
        case "permission.updated": {
          if (evt.properties.sessionID === sid) {
            patch({ pendingPermission: evt.properties })
          }
          break
        }

        // --- Permission replied (clear pending) ---
        case "permission.replied": {
          if (evt.properties.sessionID === sid) {
            patch({ pendingPermission: null })
          }
          break
        }

        // --- Todo list updated ---
        case "todo.updated": {
          if (evt.properties.sessionID === sid) {
            patch({ todos: evt.properties.todos })
          }
          break
        }

        // --- Session created ---
        case "session.created": {
          if (evt.properties.info.id === sid) {
            patch({ session: evt.properties.info })
          }
          break
        }

        // --- Session updated (title, summary, etc.) ---
        case "session.updated": {
          if (evt.properties.info.id === sid) {
            patch({ session: evt.properties.info })
          }
          break
        }

        // --- Session error ---
        case "session.error": {
          if (
            evt.properties.sessionID === sid ||
            evt.properties.sessionID === undefined
          ) {
            patch({ error: evt })
          }
          break
        }

        // --- File edited ---
        case "file.edited": {
          setState((prev) => {
            const file = evt.properties.file
            if (prev.editedFiles.includes(file)) return prev
            return { ...prev, editedFiles: [...prev.editedFiles, file] }
          })
          break
        }

        // --- Message removed ---
        case "message.removed": {
          if (evt.properties.sessionID === sid) {
            setState((prev) => ({
              ...prev,
              messages: prev.messages.filter(
                (m) => m.info.id !== evt.properties.messageID,
              ),
            }))
          }
          break
        }

        // --- Message part removed ---
        case "message.part.removed": {
          if (evt.properties.sessionID === sid) {
            setState((prev) => ({
              ...prev,
              messages: prev.messages.map((m) =>
                m.info.id === evt.properties.messageID
                  ? {
                      ...m,
                      parts: m.parts.filter(
                        (p) => p.id !== evt.properties.partID,
                      ),
                    }
                  : m,
              ),
            }))
          }
          break
        }

        // --- Session compacted (messages were compacted, reload) ---
        case "session.compacted": {
          if (evt.properties.sessionID === sid) {
            // Reload messages after compaction
            void (async () => {
              try {
                const messagesRes = await client!.session.messages({
                  path: { id: sid },
                  query: q,
                })
                const messages: SessionMessage[] = (
                  messagesRes.data ?? []
                ).map((m) => ({ info: m.info, parts: m.parts }))
                patch({ messages })
              } catch {
                // non-critical
              }
            })()
          }
          break
        }

        // --- Session diff (real-time diff update) ---
        case "session.diff": {
          if (evt.properties.sessionID === sid) {
            patch({ diffs: evt.properties.diff })
          }
          break
        }

        // --- Session idle (convenience – also covered by session.status) ---
        case "session.idle": {
          if (evt.properties.sessionID === sid) {
            patch({ status: { type: "idle" } })
          }
          break
        }

        default:
          // Ignore events we don't handle
          break
      }
    }

    subscribe()
    return () => {
      cancelled = true
    }
  }, [client, sessionId, directory, patch, upsertMessage, upsertPart])

  // -----------------------------------------------------------------------
  // Actions
  // -----------------------------------------------------------------------

  const sendMessage = useCallback(
    async (text: string) => {
      if (!client || !sessionId) return
      const q = directory ? { directory } : undefined
      await client.session.promptAsync({
        path: { id: sessionId },
        query: q,
        body: {
          parts: [{ type: "text", text }],
        },
      })
    },
    [client, sessionId, directory],
  )

  const abort = useCallback(async () => {
    if (!client || !sessionId) return
    const q = directory ? { directory } : undefined
    await client.session.abort({
      path: { id: sessionId },
      query: q,
    })
  }, [client, sessionId, directory])

  const replyPermission = useCallback(
    async (
      permissionId: string,
      response: "once" | "always" | "reject",
    ) => {
      if (!client || !sessionId) return
      const q = directory ? { directory } : undefined
      await client.postSessionIdPermissionsPermissionId({
        path: { id: sessionId, permissionID: permissionId },
        query: q,
        body: { response },
      })
      patch({ pendingPermission: null })
    },
    [client, sessionId, directory, patch],
  )

  return {
    state,
    actions: { sendMessage, abort, replyPermission },
  }
}
