import React from "react"
import { Box, Text } from "ink"
import type { IKanbanTask } from "../state/types.ts"

interface CardProps {
  task: IKanbanTask
  isSelected: boolean
}

const STATUS_BADGE: Record<string, string> = {
  Todo: "○",
  InProgress: "◑",
  InReview: "◕",
  Done: "●",
}

export function Card({ task, isSelected }: CardProps) {
  const badge = STATUS_BADGE[task.status] ?? "○"

  return (
    <Box
      borderStyle={isSelected ? "bold" : "single"}
      borderColor={isSelected ? "cyan" : "gray"}
      flexDirection="column"
      paddingX={1}
      width="100%"
    >
      <Box gap={1}>
        <Text color="blue">{badge}</Text>
        <Text bold wrap="truncate">
          {task.title}
        </Text>
      </Box>

      {task.description ? (
        <Text color="gray" dimColor wrap="truncate">
          {task.description}
        </Text>
      ) : null}

      {task.sessionId ? (
        <Text color="green" dimColor>
          ⚡ session active
        </Text>
      ) : null}

      {task.branchName ? (
        <Text color="yellow" dimColor wrap="truncate">
          ⎇ {task.branchName}
        </Text>
      ) : null}
    </Box>
  )
}
