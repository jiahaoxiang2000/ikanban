import { $ } from "bun"
import { join } from "node:path"
import { existsSync } from "node:fs"
import { type AgentInstance, createAgent, destroyAgent } from "./instance.ts"

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
): Promise<{ worktreePath: string; branchName: string }> {
  const branchName = branchFor(taskId, title)
  const worktreePath = worktreeDir(projectPath, taskId)

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
    return { worktreePath, branchName }
  }

  // Check if the branch already exists
  try {
    await $`git -C ${projectPath} rev-parse --verify ${branchName}`.quiet()
    // Branch exists but worktree doesn't – create worktree from existing branch
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

  return { worktreePath, branchName }
}

export async function removeWorktree(
  projectPath: string,
  worktreePath: string,
  branchName: string,
): Promise<void> {
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
): Promise<AgentInstance> {
  const existing = agents.get(taskId)
  if (existing) return existing

  const { worktreePath, branchName } = await addWorktree(
    projectPath,
    taskId,
    title,
  )

  try {
    const agent = await createAgent(taskId, worktreePath, branchName)
    agents.set(taskId, agent)
    return agent
  } catch (err) {
    // Clean up the worktree if SDK initialization failed
    try {
      await removeWorktree(projectPath, worktreePath, branchName)
    } catch {
      // best-effort cleanup
    }
    throw new AgentError(
      `Failed to create opencode agent: ${err instanceof Error ? err.message : String(err)}`,
      "sdk",
      err,
    )
  }
}

export async function stopAgent(
  projectPath: string,
  taskId: string,
): Promise<void> {
  const agent = agents.get(taskId)
  if (!agent) return

  await destroyAgent(agent)
  agents.delete(taskId)

  await removeWorktree(projectPath, agent.worktreePath, agent.branchName)
}

export async function stopAllAgents(projectPath: string): Promise<void> {
  const entries = [...agents.entries()]
  await Promise.allSettled(
    entries.map(([taskId]) => stopAgent(projectPath, taskId)),
  )
}
