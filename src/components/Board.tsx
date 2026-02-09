import React from "react"
import { Box } from "ink"
import { TASK_COLUMNS } from "../state/types.ts"
import { store, useStore } from "../state/store.ts"
import { Column } from "./Column.tsx"

export function Board() {
  const { tasks, view, columnIndex, selectedIndex } = useStore()

  if (view.kind !== "tasks") return null

  const projectId = view.projectId

  return (
    <Box flexDirection="row" width="100%" flexGrow={1}>
      {TASK_COLUMNS.map((status, colIdx) => {
        const columnTasks = store.getTasksByColumn(projectId, status)
        return (
          <Column
            key={status}
            status={status}
            tasks={columnTasks}
            isActive={colIdx === columnIndex}
            selectedIndex={colIdx === columnIndex ? selectedIndex : -1}
          />
        )
      })}
    </Box>
  )
}
