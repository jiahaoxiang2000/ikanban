import { createOpencode } from "@opencode-ai/sdk"
import type { OpencodeClient } from "@opencode-ai/sdk"
import { runtimeLog } from "../state/storage.ts"

interface RuntimeState {
  client: OpencodeClient
  server: { url: string; close(): void }
}

let runtime: RuntimeState | null = null
let starting: Promise<RuntimeState> | null = null

export async function startRuntime(): Promise<OpencodeClient> {
  if (runtime) {
    runtimeLog.debug("Reusing shared opencode runtime", {
      source: "runtime.start",
      url: runtime.server.url,
    })
    return runtime.client
  }
  if (starting) {
    runtimeLog.debug("Waiting for shared runtime startup", {
      source: "runtime.start",
    })
    const pending = await starting
    return pending.client
  }

  starting = (async () => {
    runtimeLog.info("Starting shared opencode runtime", {
      source: "runtime.start",
    })
    const result = await createOpencode({ port: 0 })
    runtime = { client: result.client, server: result.server }
    runtimeLog.info("Shared opencode runtime started", {
      source: "runtime.start",
      url: result.server.url,
    })
    return runtime
  })()

  try {
    const started = await starting
    return started.client
  } catch (err) {
    runtimeLog.error("Failed to start shared opencode runtime", {
      source: "runtime.start",
      err,
    })
    throw err
  } finally {
    starting = null
  }
}

export function getRuntimeClient(): OpencodeClient | null {
  return runtime?.client ?? null
}

export function getRuntimeServerUrl(): string | null {
  return runtime?.server.url ?? null
}

export async function stopRuntime(): Promise<void> {
  if (!runtime) return
  runtimeLog.info("Stopping shared opencode runtime", {
    source: "runtime.stop",
    url: runtime.server.url,
  })
  runtime.server.close()
  runtime = null
}
