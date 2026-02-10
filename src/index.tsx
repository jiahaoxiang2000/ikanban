#!/usr/bin/env bun
import { render } from "ink"
import React from "react"
import { App } from "./app.tsx"
import { stopAllAgents } from "./agent/registry.ts"
import { store } from "./state/store.ts"

const instance = render(<App />)

async function cleanup() {
  // Find any project path to use for worktree cleanup
  const state = store.getState()
  for (const project of state.projects) {
    await stopAllAgents(project.path)
  }
  instance.unmount()
}

process.on("SIGINT", () => {
  void cleanup().then(() => process.exit(0))
})

process.on("SIGTERM", () => {
  void cleanup().then(() => process.exit(0))
})
