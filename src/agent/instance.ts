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
  const { client, server } = await createOpencode()

  const { data: session } = await client.session.create({
    body: {},
    query: { directory: worktreePath },
  })

  if (!session) {
    server.close()
    throw new Error(`Failed to create session for task ${taskId}`)
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
