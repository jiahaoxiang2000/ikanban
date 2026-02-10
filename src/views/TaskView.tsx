import React, { useState, useCallback } from "react"
import { Box, Text } from "ink"
import { store, useStore } from "../state/store.ts"
import { TASK_COLUMNS } from "../state/types.ts"
import { useKeyboard, clampIndex, getNextColumnStatus } from "../hooks/useKeyboard.ts"
import { Board } from "../components/Board.tsx"
import { Input } from "../components/Input.tsx"
import { startAgent } from "../agent/registry.ts"

export function TaskView() {
  const { view, tasks, projects, selectedIndex, columnIndex, inputFocused } =
    useStore()
  const [inputMode, setInputMode] = useState<"none" | "new" | "edit">("none")

  if (view.kind !== "tasks") return null

  const projectId = view.projectId
  const project = projects.find((p) => p.id === projectId)
  const currentStatus = TASK_COLUMNS[columnIndex]!
  const columnTasks = store.getTasksByColumn(projectId, currentStatus)
  const selectedTask = columnTasks[selectedIndex]

  const handleNew = useCallback(() => {
    setInputMode("new")
    store.setInputFocused(true)
  }, [])

  const handleEdit = useCallback(() => {
    if (!selectedTask) return
    setInputMode("edit")
    store.setInputFocused(true)
  }, [selectedTask])

  const handleDelete = useCallback(() => {
    if (!selectedTask) return
    store.deleteTask(selectedTask.id)
    store.setSelectedIndex(
      clampIndex(selectedIndex, Math.max(0, columnTasks.length - 2)),
    )
  }, [selectedTask, selectedIndex, columnTasks.length])

  const handleSelect = useCallback(async () => {
    if (!selectedTask || !project) return

    // If the task already has a session, just navigate to it
    if (selectedTask.sessionId) {
      store.navigate({
        kind: "session",
        taskId: selectedTask.id,
        sessionId: selectedTask.sessionId,
      })
      return
    }

    // Otherwise create worktree + agent + session, then navigate
    try {
      const agent = await startAgent(
        project.path,
        selectedTask.id,
        selectedTask.title,
      )
      store.updateTask(selectedTask.id, {
        sessionId: agent.sessionId,
        worktreePath: agent.worktreePath,
        branchName: agent.branchName,
      })
      store.navigate({
        kind: "session",
        taskId: selectedTask.id,
        sessionId: agent.sessionId,
      })
    } catch (err) {
      // Could not start agent - stay on task view
    }
  }, [selectedTask, project])

  const handleMoveRight = useCallback(() => {
    if (!selectedTask) return
    const nextStatus = getNextColumnStatus(selectedTask.status)
    if (nextStatus) {
      store.updateTask(selectedTask.id, { status: nextStatus })
    }
  }, [selectedTask])

  const handleSubmit = useCallback(
    (value: string) => {
      if (inputMode === "new") {
        store.addTask(projectId, value)
        setInputMode("none")
        store.setInputFocused(false)
        // Select the newly added task (it goes to Todo column)
        if (columnIndex === 0) {
          const todoTasks = store.getTasksByColumn(projectId, "Todo")
          store.setSelectedIndex(todoTasks.length - 1)
        }
      } else if (inputMode === "edit" && selectedTask) {
        store.updateTask(selectedTask.id, { title: value })
        setInputMode("none")
        store.setInputFocused(false)
      }
    },
    [inputMode, projectId, columnIndex, selectedTask],
  )

  useKeyboard({
    onNavigateUp: () => {
      store.setSelectedIndex(
        clampIndex(selectedIndex - 1, Math.max(0, columnTasks.length - 1)),
      )
    },
    onNavigateDown: () => {
      store.setSelectedIndex(
        clampIndex(selectedIndex + 1, Math.max(0, columnTasks.length - 1)),
      )
    },
    onNavigateLeft: () => {
      if (columnIndex > 0) {
        const newColIndex = columnIndex - 1
        store.setColumnIndex(newColIndex)
        const newColTasks = store.getTasksByColumn(
          projectId,
          TASK_COLUMNS[newColIndex]!,
        )
        store.setSelectedIndex(
          clampIndex(selectedIndex, Math.max(0, newColTasks.length - 1)),
        )
      }
    },
    onNavigateRight: () => {
      if (columnIndex < TASK_COLUMNS.length - 1) {
        const newColIndex = columnIndex + 1
        store.setColumnIndex(newColIndex)
        const newColTasks = store.getTasksByColumn(
          projectId,
          TASK_COLUMNS[newColIndex]!,
        )
        store.setSelectedIndex(
          clampIndex(selectedIndex, Math.max(0, newColTasks.length - 1)),
        )
      }
    },
    onSelect: () => {
      void handleSelect()
    },
    onGoBack: () => {
      if (inputMode !== "none") {
        setInputMode("none")
        store.setInputFocused(false)
      } else {
        store.navigate({ kind: "projects" })
      }
    },
    onNew: handleNew,
    onDelete: handleDelete,
    onEdit: handleEdit,
    onMoveTaskRight: handleMoveRight,
    onToggleHelp: () => store.toggleLogs(),
  })

  return (
    <Box flexDirection="column" flexGrow={1}>
      <Box paddingX={1} marginBottom={0}>
        <Text color="gray">{"< "}</Text>
        <Text bold color="cyan">
          {project?.name ?? "Unknown Project"}
        </Text>
        <Text color="gray"> - Tasks</Text>
      </Box>

      <Board />

      {inputMode !== "none" && (
        <Box paddingX={1}>
          <Input
            placeholder={
              inputMode === "new" ? "New task title…" : "Edit task title…"
            }
            onSubmit={handleSubmit}
          />
        </Box>
      )}

      <Box paddingX={1}>
        <Text color="gray" dimColor>
          [n] new  [e] edit  [d] delete  [r] move right  [Enter/l] open session
           [Esc] back  [?] help
        </Text>
      </Box>
    </Box>
  )
}
