import { $ } from "bun"
import { join } from "node:path"
import { existsSync } from "node:fs"
import { type AgentInstance, createAgent, destroyAgent } from "./instance.ts"
import { startRuntime } from "./runtime.ts"
import { appendRuntimeLog } from "../state/storage.ts"

const agents = new Map<string, AgentInstance>()

/** Structured error for agent/worktree operations */
export class AgentError extends Error {
  readonly code: "worktree" | "sdk" | "session" | "cleanup"

  constructor(
    message: string,
    code: "worktree" | "sdk" | "session" | "cleanup",
    override readonly cause?: unknown,
  ) {
    super(message)
    this.name = "AgentError"
    this.code = code
  }
}

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 40)
}

function branchFor(taskId: string, title: string): string {
  return `task/${taskId.slice(0, 8)}-${slugify(title)}`
}

function worktreeDir(projectPath: string, taskId: string): string {
  return join(projectPath, "..", `.worktree-${taskId.slice(0, 8)}`)
}

export async function addWorktree(
  projectPath: string,
  taskId: string,
  title: string,
  preferredBranchName?: string,
): Promise<{ worktreePath: string; branchName: string; created: boolean }> {
  const branchName = preferredBranchName ?? branchFor(taskId, title)
  const worktreePath = worktreeDir(projectPath, taskId)

  appendRuntimeLog("debug", "Preparing worktree", {
    source: "agent.registry.addWorktree",
    projectPath,
    taskId,
    branchName,
    worktreePath,
  })

  // Verify the project path is a git repository
  if (!existsSync(projectPath)) {
    throw new AgentError(
      `Project path does not exist: ${projectPath}`,
      "worktree",
    )
  }

  try {
    await $`git -C ${projectPath} rev-parse --git-dir`.quiet()
  } catch (err) {
    throw new AgentError(
      `Not a git repository: ${projectPath}`,
      "worktree",
      err,
    )
  }

  if (existsSync(worktreePath)) {
    // Worktree already exists (e.g. app restarted) – reuse it
    appendRuntimeLog("info", "Reusing existing worktree", {
      source: "agent.registry.addWorktree",
      taskId,
      branchName,
      worktreePath,
    })
    return { worktreePath, branchName, created: false }
  }

  // Check if the branch already exists
  try {
    await $`git -C ${projectPath} rev-parse --verify ${branchName}`.quiet()
    // Branch exists but worktree doesn't – create worktree from existing branch
    appendRuntimeLog("debug", "Branch exists; creating worktree from existing branch", {
      source: "agent.registry.addWorktree",
      taskId,
      branchName,
      worktreePath,
    })
    try {
      await $`git -C ${projectPath} worktree add ${worktreePath} ${branchName}`.quiet()
    } catch (err) {
      throw new AgentError(
        `Failed to create worktree from existing branch '${branchName}': ${err instanceof Error ? err.message : String(err)}`,
        "worktree",
        err,
      )
    }
  } catch (err) {
    if (err instanceof AgentError) throw err
    // Branch doesn't exist – create both
    appendRuntimeLog("debug", "Branch missing; creating worktree with new branch", {
      source: "agent.registry.addWorktree",
      taskId,
      branchName,
      worktreePath,
    })
    try {
      await $`git -C ${projectPath} worktree add ${worktreePath} -b ${branchName}`.quiet()
    } catch (innerErr) {
      throw new AgentError(
        `Failed to create worktree with new branch '${branchName}': ${innerErr instanceof Error ? innerErr.message : String(innerErr)}`,
        "worktree",
        innerErr,
      )
    }
  }

  appendRuntimeLog("info", "Worktree created", {
    source: "agent.registry.addWorktree",
    taskId,
    branchName,
    worktreePath,
  })

  return { worktreePath, branchName, created: true }
}

export async function removeWorktree(
  projectPath: string,
  worktreePath: string,
  branchName: string,
): Promise<void> {
  appendRuntimeLog("debug", "Removing worktree", {
    source: "agent.registry.removeWorktree",
    projectPath,
    worktreePath,
    branchName,
  })
  try {
    await $`git -C ${projectPath} worktree remove ${worktreePath} --force`.quiet()
  } catch (err) {
    throw new AgentError(
      `Failed to remove worktree at ${worktreePath}: ${err instanceof Error ? err.message : String(err)}`,
      "cleanup",
      err,
    )
  }
  try {
    await $`git -C ${projectPath} branch -D ${branchName}`.quiet()
  } catch {
    // branch may have been merged already
  }
}

export function getAgent(taskId: string): AgentInstance | undefined {
  return agents.get(taskId)
}

export function listAgents(): AgentInstance[] {
  return [...agents.values()]
}

export async function startAgent(
  projectPath: string,
  taskId: string,
  title: string,
  options?: { sessionId?: string; branchName?: string },
): Promise<AgentInstance> {
  appendRuntimeLog("info", "Starting agent", {
    source: "agent.registry.startAgent",
    taskId,
    projectPath,
    title,
    options,
  })

  const existing = agents.get(taskId)
  if (existing) {
    appendRuntimeLog("debug", "Agent already exists in registry", {
      source: "agent.registry.startAgent",
      taskId,
      sessionId: existing.sessionId,
    })
    return existing
  }

  const { worktreePath, branchName, created } = await addWorktree(
    projectPath,
    taskId,
    title,
    options?.branchName,
  )

  try {
    const client = await startRuntime()
    const agent = await createAgent(
      taskId,
      projectPath,
      worktreePath,
      branchName,
      client,
      options?.sessionId,
    )
    agents.set(taskId, agent)
    appendRuntimeLog("info", "Agent started", {
      source: "agent.registry.startAgent",
      taskId,
      sessionId: agent.sessionId,
      worktreePath: agent.worktreePath,
      branchName: agent.branchName,
    })
    return agent
  } catch (err) {
    // Clean up the worktree if SDK initialization failed
    if (created) {
      try {
        await removeWorktree(projectPath, worktreePath, branchName)
      } catch {
        // best-effort cleanup
      }
    }
    appendRuntimeLog("error", "Failed to start agent", {
      source: "agent.registry.startAgent",
      taskId,
      err,
    })
    throw new AgentError(
      `Failed to create opencode agent: ${err instanceof Error ? err.message : String(err)}`,
      "sdk",
      err,
    )
  }
}

export async function stopAgent(
  taskId: string,
): Promise<void> {
  const agent = agents.get(taskId)
  if (!agent) return

  appendRuntimeLog("info", "Stopping agent", {
    source: "agent.registry.stopAgent",
    taskId,
    sessionId: agent.sessionId,
    worktreePath: agent.worktreePath,
  })

  await destroyAgent(agent)
  agents.delete(taskId)

  await removeWorktree(agent.projectPath, agent.worktreePath, agent.branchName)

  appendRuntimeLog("info", "Agent stopped", {
    source: "agent.registry.stopAgent",
    taskId,
  })
}

export async function stopAllAgents(): Promise<void> {
  const entries = [...agents.entries()]
  appendRuntimeLog("info", "Stopping all agents", {
    source: "agent.registry.stopAllAgents",
    count: entries.length,
  })
  await Promise.allSettled(
    entries.map(([taskId]) => stopAgent(taskId)),
  )
}
