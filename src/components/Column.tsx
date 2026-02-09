import React from "react"
import { Box, Text } from "ink"
import type { IKanbanTask, TaskStatus } from "../state/types.ts"
import { Card } from "./Card.tsx"

interface ColumnProps {
  status: TaskStatus
  tasks: IKanbanTask[]
  isActive: boolean
  selectedIndex: number
}

const COLUMN_COLORS: Record<TaskStatus, string> = {
  Todo: "white",
  InProgress: "cyan",
  InReview: "yellow",
  Done: "green",
}

const COLUMN_LABELS: Record<TaskStatus, string> = {
  Todo: "To Do",
  InProgress: "In Progress",
  InReview: "In Review",
  Done: "Done",
}

export function Column({ status, tasks, isActive, selectedIndex }: ColumnProps) {
  const color = COLUMN_COLORS[status]
  const label = COLUMN_LABELS[status]

  return (
    <Box
      flexDirection="column"
      flexGrow={1}
      flexBasis={0}
      borderStyle="round"
      borderColor={isActive ? color : "gray"}
      paddingX={1}
    >
      <Box justifyContent="center" marginBottom={1}>
        <Text bold color={color}>
          {label}
        </Text>
        <Text color="gray"> ({tasks.length})</Text>
      </Box>

      {tasks.length === 0 ? (
        <Text color="gray" dimColor>
          No tasks
        </Text>
      ) : (
        tasks.map((task, i) => (
          <Card
            key={task.id}
            task={task}
            isSelected={isActive && i === selectedIndex}
          />
        ))
      )}
    </Box>
  )
}
