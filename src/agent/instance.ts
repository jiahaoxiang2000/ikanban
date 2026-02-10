import { createOpencode } from "@opencode-ai/sdk"
import type { OpencodeClient } from "@opencode-ai/sdk"

export interface AgentInstance {
  taskId: string
  worktreePath: string
  branchName: string
  sessionId: string
  client: OpencodeClient
  server: { url: string; close(): void }
}

export async function createAgent(
  taskId: string,
  worktreePath: string,
  branchName: string,
): Promise<AgentInstance> {
  let client: OpencodeClient
  let server: { url: string; close(): void }

  try {
    const result = await createOpencode()
    client = result.client
    server = result.server
  } catch (err) {
    throw new Error(
      `Failed to start opencode server: ${err instanceof Error ? err.message : String(err)}`,
    )
  }

  let session: { id: string } | undefined
  try {
    const result = await client.session.create({
      body: {},
      query: { directory: worktreePath },
    })
    session = result.data ?? undefined
  } catch (err) {
    server.close()
    throw new Error(
      `Failed to create session for task ${taskId}: ${err instanceof Error ? err.message : String(err)}`,
    )
  }

  if (!session) {
    server.close()
    throw new Error(`Failed to create session for task ${taskId}: no session returned`)
  }

  return {
    taskId,
    worktreePath,
    branchName,
    sessionId: session.id,
    client,
    server,
  }
}

export async function destroyAgent(agent: AgentInstance): Promise<void> {
  try {
    await agent.client.session.abort({
      path: { id: agent.sessionId },
      query: { directory: agent.worktreePath },
    })
  } catch {
    // session may already be finished
  }
  agent.server.close()
}
