import React, { useEffect, useMemo } from "react"
import { Box, Text, useInput } from "ink"
import { LogPanel } from "./components/LogPanel.tsx"
import { store, useStore } from "./state/store.ts"
import { TASK_COLUMNS, type AppView } from "./state/types.ts"
import { clampIndex } from "./hooks/useKeyboard.ts"
import { ProjectView } from "./views/ProjectView.tsx"
import { SessionView } from "./views/SessionView.tsx"
import { TaskView } from "./views/TaskView.tsx"

function goBack(view: AppView) {
  if (view.kind === "tasks") {
    store.navigate({ kind: "projects" })
    return
  }

  if (view.kind === "session") {
    const state = store.getState()
    const task = state.tasks.find((item) => item.id === view.taskId)
    if (task) {
      store.navigate({ kind: "tasks", projectId: task.projectId })
    } else {
      store.navigate({ kind: "projects" })
    }
  }
}

export function App() {
  const { view, projects, selectedIndex, columnIndex, showLogs, inputFocused } =
    useStore()

  useEffect(() => {
    if (view.kind === "projects") {
      if (columnIndex !== 0) {
        store.setColumnIndex(0)
      }
      const maxIndex = Math.max(0, projects.length - 1)
      const nextIndex = clampIndex(selectedIndex, maxIndex)
      if (nextIndex !== selectedIndex) {
        store.setSelectedIndex(nextIndex)
      }
      return
    }

    if (view.kind === "tasks") {
      const maxColumn = TASK_COLUMNS.length - 1
      const nextColumn = clampIndex(columnIndex, maxColumn)
      if (nextColumn !== columnIndex) {
        store.setColumnIndex(nextColumn)
      }

      const status = TASK_COLUMNS[nextColumn]
      if (!status) return
      const columnTasks = store.getTasksByColumn(view.projectId, status)
      const maxIndex = Math.max(0, columnTasks.length - 1)
      const nextIndex = clampIndex(selectedIndex, maxIndex)
      if (nextIndex !== selectedIndex) {
        store.setSelectedIndex(nextIndex)
      }
      return
    }

    if (columnIndex !== 0) {
      store.setColumnIndex(0)
    }
    if (selectedIndex !== 0) {
      store.setSelectedIndex(0)
    }
    if (inputFocused) {
      store.setInputFocused(false)
    }
  }, [view, projects.length, selectedIndex, columnIndex, inputFocused])

  useInput((input, key) => {
    if (inputFocused) return
    if (view.kind !== "session") return

    if (key.escape || input === "h") {
      goBack(view)
      return
    }

    if (input === "?") {
      store.toggleLogs()
    }
  })

  const title = useMemo(() => {
    if (view.kind === "projects") return "Projects"
    if (view.kind === "tasks") return "Tasks"
    return "Session"
  }, [view])

  return (
    <Box flexDirection="column" width="100%" height="100%">
      <Box borderStyle="round" borderColor="cyan" paddingX={1} justifyContent="center">
        <Text bold color="cyan">
          iKanban
        </Text>
        <Text color="gray"> {title}</Text>
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
