import { useState, useEffect, useCallback, useRef } from "react"
import { createOpencode } from "@opencode-ai/sdk"
import type { OpencodeClient } from "@opencode-ai/sdk"
import { useSession, type UseSessionResult } from "./useSession.ts"
import { store } from "../state/store.ts"
import {
  startAgent,
  stopAgent,
  getAgent,
} from "../agent/registry.ts"

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type AgentPhase =
  | "idle"       // no agent running for this task
  | "starting"   // creating worktree + opencode server
  | "ready"      // agent running, session connected
  | "stopping"   // tearing down
  | "error"      // something went wrong

export interface UseAgentResult {
  /** Current lifecycle phase */
  phase: AgentPhase
  /** The underlying opencode client (null until ready) */
  client: OpencodeClient | null
  /** Session hook result (state + actions) for the active session */
  session: UseSessionResult
  /** Start the agent for this task (creates worktree + opencode server + sends initial prompt) */
  start: (initialPrompt?: string) => Promise<void>
  /** Stop the agent and clean up */
  stop: () => Promise<void>
  /** Send a prompt to the agent's session */
  sendMessage: (text: string) => Promise<void>
  /** Last error message, if phase === "error" */
  error: string | null
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildInitialPrompt(
  task: { title: string; description?: string } | undefined,
): string | null {
  if (!task) return null
  const parts = [task.title]
  if (task.description) parts.push(task.description)
  return parts.join("\n\n")
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Manages the full `createOpencode()` lifecycle for a single kanban task.
 *
 * - Creates a git worktree + opencode server on `start()`
 * - Provides the `OpencodeClient` and wires it into `useSession`
 * - Tears everything down on `stop()` or unmount
 *
 * @param taskId      – The kanban task ID
 * @param projectPath – Root git repo path (used for worktree creation)
 */
export function useAgent(
  taskId: string | null,
  projectPath: string | null,
): UseAgentResult {
  const [phase, setPhase] = useState<AgentPhase>("idle")
  const [client, setClient] = useState<OpencodeClient | null>(null)
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [directory, setDirectory] = useState<string | undefined>(undefined)
  const [error, setError] = useState<string | null>(null)

  // Track whether the component is still mounted
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  // On mount, check if an agent already exists for this task (e.g. after
  // navigating away and back).
  useEffect(() => {
    if (!taskId) return
    const existing = getAgent(taskId)
    if (existing) {
      setClient(existing.client)
      setSessionId(existing.sessionId)
      setDirectory(existing.worktreePath)
      setPhase("ready")
    }
  }, [taskId])

  // Wire up the session hook
  const session = useSession(client, sessionId, directory)

  // -----------------------------------------------------------------------
  // Start
  // -----------------------------------------------------------------------

  /**
   * Start the agent for this task. If `initialPrompt` is provided, it will be
   * sent to the session immediately after creation (the task's
   * title + description are used by default when called from the UI).
   */
  const start = useCallback(async (initialPrompt?: string) => {
    if (!taskId || !projectPath) return
    if (phase === "starting" || phase === "ready") return

    setPhase("starting")
    setError(null)

    try {
      // Look up the task title for branch naming
      const appState = store.getState()
      const task = appState.tasks.find((t) => t.id === taskId)
      const title = task?.title ?? "task"

      const agent = await startAgent(projectPath, taskId, title)

      if (!mountedRef.current) {
        // Component unmounted while we were starting – clean up
        await stopAgent(projectPath, taskId)
        return
      }

      // Persist agent metadata on the task
      store.updateTask(taskId, {
        sessionId: agent.sessionId,
        worktreePath: agent.worktreePath,
        branchName: agent.branchName,
        status: "InProgress",
      })

      setClient(agent.client)
      setSessionId(agent.sessionId)
      setDirectory(agent.worktreePath)
      setPhase("ready")

      // Send the initial prompt if provided
      const prompt = initialPrompt ?? buildInitialPrompt(task)
      if (prompt) {
        await agent.client.session.promptAsync({
          path: { id: agent.sessionId },
          query: { directory: agent.worktreePath },
          body: { parts: [{ type: "text", text: prompt }] },
        })
      }
    } catch (err) {
      if (mountedRef.current) {
        setPhase("error")
        setError(
          err instanceof Error ? err.message : "Failed to start agent",
        )
      }
    }
  }, [taskId, projectPath, phase])

  // -----------------------------------------------------------------------
  // Stop
  // -----------------------------------------------------------------------

  const stop = useCallback(async () => {
    if (!taskId || !projectPath) return
    if (phase === "idle" || phase === "stopping") return

    setPhase("stopping")

    try {
      await stopAgent(projectPath, taskId)
    } catch {
      // best-effort cleanup
    }

    if (mountedRef.current) {
      setClient(null)
      setSessionId(null)
      setDirectory(undefined)
      setPhase("idle")
    }
  }, [taskId, projectPath, phase])

  // -----------------------------------------------------------------------
  // Cleanup on unmount – do NOT destroy the agent so it can be resumed
  // -----------------------------------------------------------------------

  // (Intentionally no cleanup – the agent registry keeps it alive so the
  // user can navigate away and come back. Call `stop()` explicitly or
  // `stopAllAgents()` on app exit.)

  // -----------------------------------------------------------------------
  // Convenience: forward sendMessage
  // -----------------------------------------------------------------------

  const sendMessage = useCallback(
    async (text: string) => {
      await session.actions.sendMessage(text)
    },
    [session.actions],
  )

  return {
    phase,
    client,
    session,
    start,
    stop,
    sendMessage,
    error,
  }
}
