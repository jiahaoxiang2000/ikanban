import type { OpencodeClient } from "@opencode-ai/sdk"
import { appendRuntimeLog } from "../state/storage.ts"

export interface AgentInstance {
  taskId: string
  projectPath: string
  worktreePath: string
  branchName: string
  sessionId: string
  client: OpencodeClient
}

export async function createAgent(
  taskId: string,
  projectPath: string,
  worktreePath: string,
  branchName: string,
  client: OpencodeClient,
  existingSessionId?: string,
): Promise<AgentInstance> {
  appendRuntimeLog("info", "Creating agent instance", {
    source: "agent.instance.create",
    taskId,
    worktreePath,
    branchName,
    existingSessionId,
  })

  let session: { id: string } | undefined
  const query = { directory: worktreePath }

  if (existingSessionId) {
    appendRuntimeLog("debug", "Checking existing session id", {
      source: "agent.instance.create",
      taskId,
      existingSessionId,
    })
    try {
      const existing = await client.session.get({
        path: { id: existingSessionId },
        query,
      })
      if (existing.data) {
        session = { id: existing.data.id }
        appendRuntimeLog("info", "Reused existing session", {
          source: "agent.instance.create",
          taskId,
          sessionId: existing.data.id,
        })
      }
    } catch {
      // stale session id; fall through and create a new one
      appendRuntimeLog("warn", "Stored session id is stale; creating new session", {
        source: "agent.instance.create",
        taskId,
        existingSessionId,
      })
    }
  }

  if (!session) {
    appendRuntimeLog("debug", "Creating new session for agent", {
      source: "agent.instance.create",
      taskId,
      worktreePath,
    })
    try {
      const result = await client.session.create({
        body: {},
        query,
      })
      session = result.data ?? undefined
      appendRuntimeLog("info", "Created new session", {
        source: "agent.instance.create",
        taskId,
        sessionId: session?.id,
      })
    } catch (err) {
      appendRuntimeLog("error", "Failed to create session", {
        source: "agent.instance.create",
        taskId,
        err,
      })
      throw new Error(
        `Failed to create session for task ${taskId}: ${err instanceof Error ? err.message : String(err)}`,
      )
    }
  }

  if (!session) {
    throw new Error(`Failed to create session for task ${taskId}: no session returned`)
  }

  return {
    taskId,
    projectPath,
    worktreePath,
    branchName,
    sessionId: session.id,
    client,
  }
}

export async function destroyAgent(agent: AgentInstance): Promise<void> {
  appendRuntimeLog("info", "Destroying agent instance", {
    source: "agent.instance.destroy",
    taskId: agent.taskId,
    sessionId: agent.sessionId,
    worktreePath: agent.worktreePath,
  })
  try {
    await agent.client.session.abort({
      path: { id: agent.sessionId },
      query: { directory: agent.worktreePath },
    })
    appendRuntimeLog("debug", "Abort session request sent", {
      source: "agent.instance.destroy",
      taskId: agent.taskId,
      sessionId: agent.sessionId,
    })
  } catch {
    // session may already be finished
    appendRuntimeLog("debug", "Abort skipped; session already finished", {
      source: "agent.instance.destroy",
      taskId: agent.taskId,
      sessionId: agent.sessionId,
    })
  }
}
