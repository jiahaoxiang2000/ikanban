#!/usr/bin/env bun
import { render } from "ink"
import React from "react"
import { App } from "./app.tsx"
import { stopAllAgents, listAgents } from "./agent/registry.ts"
import { store } from "./state/store.ts"

const instance = render(<App />)

let cleaningUp = false

async function cleanup() {
  if (cleaningUp) return
  cleaningUp = true

  const agents = listAgents()
  if (agents.length > 0) {
    // Give agents a grace period to shut down
    const state = store.getState()
    const cleanupPromises = state.projects.map((project) =>
      stopAllAgents(project.path),
    )

    // Race against a timeout so we don't hang forever
    await Promise.race([
      Promise.allSettled(cleanupPromises),
      new Promise((resolve) => setTimeout(resolve, 5000)),
    ])
  }

  instance.unmount()
}

/** Exported so views can call it for the `q` key quit path */
export async function gracefulExit(code = 0): Promise<void> {
  await cleanup()
  process.exit(code)
}

process.on("SIGINT", () => {
  void cleanup().then(() => process.exit(0))
})

process.on("SIGTERM", () => {
  void cleanup().then(() => process.exit(0))
})

// Handle uncaught errors gracefully
process.on("uncaughtException", (err) => {
  store.setLastError(`Uncaught error: ${err.message}`)
  // Don't exit – let the user see the error and decide
})

process.on("unhandledRejection", (reason) => {
  const message = reason instanceof Error ? reason.message : String(reason)
  store.setLastError(`Unhandled rejection: ${message}`)
})
