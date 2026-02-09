#!/usr/bin/env bun
import { render, Box, Text } from "ink"
import React from "react"
import { useStore } from "./state/store.ts"
import { ProjectView } from "./views/ProjectView.tsx"
import { TaskView } from "./views/TaskView.tsx"
import { SessionView } from "./views/SessionView.tsx"
import { LogPanel } from "./components/LogPanel.tsx"

function App() {
  const { view, showLogs } = useStore()

  return (
    <Box flexDirection="column" width="100%" height="100%">
      <Box
        borderStyle="round"
        borderColor="cyan"
        paddingX={1}
        justifyContent="center"
      >
        <Text bold color="cyan">
          iKanban
        </Text>
      </Box>

      <Box flexDirection="row" flexGrow={1}>
        <Box flexDirection="column" flexGrow={1}>
          {view.kind === "projects" && <ProjectView />}
          {view.kind === "tasks" && <TaskView />}
          {view.kind === "session" && <SessionView />}
        </Box>

        {showLogs && <LogPanel />}
      </Box>
    </Box>
  )
}

render(<App />)
