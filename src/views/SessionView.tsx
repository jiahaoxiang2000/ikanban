import React, { useState, useCallback, useEffect, useRef } from "react"
import { Box, Text, useInput } from "ink"
import { store, useStore } from "../state/store.ts"
import { useAgent } from "../hooks/useAgent.ts"
import { Input } from "../components/Input.tsx"
import type { SessionMessage } from "../hooks/useSession.ts"
import type { Part } from "@opencode-ai/sdk"

// ---------------------------------------------------------------------------
// Part renderers
// ---------------------------------------------------------------------------

function TextPartView({ part }: { part: Extract<Part, { type: "text" }> }) {
  return (
    <Box paddingLeft={2}>
      <Text wrap="wrap">{part.text}</Text>
    </Box>
  )
}

function ToolPartView({ part }: { part: Extract<Part, { type: "tool" }> }) {
  const { state } = part
  const statusColor =
    state.status === "completed"
      ? "green"
      : state.status === "error"
        ? "red"
        : state.status === "running"
          ? "yellow"
          : "gray"

  const title =
    "title" in state && state.title ? state.title : part.tool

  const statusIcon =
    state.status === "completed"
      ? "+"
      : state.status === "error"
        ? "x"
        : state.status === "running"
          ? "~"
          : "."

  return (
    <Box paddingLeft={2} flexDirection="column">
      <Text color={statusColor}>
        [{statusIcon}] {title}
      </Text>
      {state.status === "error" && "error" in state && (
        <Box paddingLeft={4}>
          <Text color="red" dimColor wrap="truncate">
            {state.error}
          </Text>
        </Box>
      )}
      {state.status === "completed" && state.output && (
        <Box paddingLeft={4}>
          <Text color="gray" dimColor wrap="truncate">
            {state.output.length > 200
              ? state.output.slice(0, 200) + "..."
              : state.output}
          </Text>
        </Box>
      )}
    </Box>
  )
}

function GenericPartView({ part }: { part: Part }) {
  return (
    <Box paddingLeft={2}>
      <Text color="gray" dimColor>
        [{part.type}]
      </Text>
    </Box>
  )
}

function PartView({ part }: { part: Part }) {
  switch (part.type) {
    case "text":
      return <TextPartView part={part} />
    case "tool":
      return <ToolPartView part={part} />
    default:
      return <GenericPartView part={part} />
  }
}

// ---------------------------------------------------------------------------
// Message renderer
// ---------------------------------------------------------------------------

function MessageView({ message }: { message: SessionMessage }) {
  const { info, parts } = message
  const isUser = info.role === "user"

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box gap={1}>
        <Text bold color={isUser ? "green" : "cyan"}>
          {isUser ? "You" : "Agent"}
        </Text>
        <Text color="gray" dimColor>
          {new Date(info.time.created).toLocaleTimeString("en-US", {
            hour12: false,
          })}
        </Text>
      </Box>
      {parts.map((part) => (
        <PartView key={part.id} part={part} />
      ))}
    </Box>
  )
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

function StatusBar({
  status,
  phase,
  error,
}: {
  status: { type: string }
  phase: string
  error: string | null
}) {
  const statusColor =
    status.type === "busy"
      ? "yellow"
      : status.type === "retry"
        ? "red"
        : "green"

  return (
    <Box paddingX={1} gap={2}>
      <Text color={statusColor} bold>
        {phase === "ready"
          ? status.type === "busy"
            ? "Working..."
            : status.type === "retry"
              ? "Retrying..."
              : "Idle"
          : phase === "starting"
            ? "Starting agent..."
            : phase === "error"
              ? "Error"
              : phase === "stopping"
                ? "Stopping..."
                : "Not started"}
      </Text>
      {error && (
        <Text color="red" dimColor>
          {error}
        </Text>
      )}
    </Box>
  )
}

// ---------------------------------------------------------------------------
// SessionView
// ---------------------------------------------------------------------------

export function SessionView() {
  const { view, showLogs, inputFocused } = useStore()
  const [scrollOffset, setScrollOffset] = useState(0)

  if (view.kind !== "session") return null

  const { taskId } = view
  const task = store.getState().tasks.find((t) => t.id === taskId)
  const project = task
    ? store.getState().projects.find((p) => p.id === task.projectId)
    : null

  const agent = useAgent(taskId, project?.path ?? null)
  const { state: sessionState, actions } = agent.session
  const { messages, status } = sessionState

  // Auto-start agent if not ready
  useEffect(() => {
    if (agent.phase === "idle" && project?.path) {
      void agent.start()
    }
  }, [agent.phase, project?.path])

  // Auto-scroll to bottom when new messages arrive
  const prevMessageCount = useRef(messages.length)
  useEffect(() => {
    if (messages.length > prevMessageCount.current) {
      setScrollOffset(0) // reset to bottom
    }
    prevMessageCount.current = messages.length
  }, [messages.length])

  // Handle prompt submission
  const handleSubmit = useCallback(
    (text: string) => {
      void agent.sendMessage(text)
    },
    [agent.sendMessage],
  )

  // Focus input on mount
  useEffect(() => {
    store.setInputFocused(true)
    return () => {
      store.setInputFocused(false)
    }
  }, [])

  // Keyboard shortcuts (active when input is NOT focused)
  useInput(
    (input, key) => {
      if (inputFocused) return

      // Ctrl+C: abort agent
      if (input === "c" && key.ctrl) {
        void actions.abort()
        return
      }

      // L: toggle log panel
      if (input === "l" || input === "L") {
        store.toggleLogs()
        return
      }

      // Escape: go back to task view
      if (key.escape) {
        if (task) {
          store.navigate({ kind: "tasks", projectId: task.projectId })
        }
        return
      }

      // i or Enter: focus input
      if (input === "i" || key.return) {
        store.setInputFocused(true)
        return
      }

      // Scroll: k/up = scroll up, j/down = scroll down
      if (input === "k" || key.upArrow) {
        setScrollOffset((prev) => Math.min(prev + 3, messages.length - 1))
        return
      }
      if (input === "j" || key.downArrow) {
        setScrollOffset((prev) => Math.max(prev - 3, 0))
        return
      }
    },
    { isActive: !inputFocused },
  )

  // Also handle Ctrl+C when input IS focused
  useInput(
    (input, key) => {
      if (input === "c" && key.ctrl) {
        void actions.abort()
      }
    },
    { isActive: inputFocused },
  )

  // Compute visible messages (show last N, offset by scroll)
  const visibleCount = 20
  const endIdx = messages.length - scrollOffset
  const startIdx = Math.max(0, endIdx - visibleCount)
  const visibleMessages = messages.slice(startIdx, endIdx)

  return (
    <Box flexDirection="column" flexGrow={1}>
      {/* Header */}
      <Box paddingX={1} gap={1}>
        <Text color="gray">{"< "}</Text>
        <Text bold color="cyan">
          {task?.title ?? "Session"}
        </Text>
        {task?.branchName && (
          <Text color="yellow" dimColor>
            [{task.branchName}]
          </Text>
        )}
      </Box>

      {/* Status bar */}
      <StatusBar
        status={status}
        phase={agent.phase}
        error={agent.error}
      />

      {/* Messages area */}
      <Box
        flexDirection="column"
        flexGrow={1}
        paddingX={1}
        overflow="hidden"
      >
        {messages.length === 0 && agent.phase === "ready" && (
          <Box padding={1}>
            <Text color="gray" dimColor>
              No messages yet. Type a prompt to get started.
            </Text>
          </Box>
        )}

        {agent.phase === "starting" && (
          <Box padding={1}>
            <Text color="yellow">
              Starting agent (creating worktree + opencode server)...
            </Text>
          </Box>
        )}

        {agent.phase === "error" && (
          <Box padding={1}>
            <Text color="red">
              Failed to start agent: {agent.error}
            </Text>
            <Text color="gray" dimColor>
              {" "}Press [Esc] to go back.
            </Text>
          </Box>
        )}

        {visibleMessages.map((msg) => (
          <MessageView key={msg.info.id} message={msg} />
        ))}

        {scrollOffset > 0 && (
          <Box justifyContent="center">
            <Text color="gray" dimColor>
              -- {scrollOffset} more below (j to scroll down) --
            </Text>
          </Box>
        )}
      </Box>

      {/* Input area */}
      {agent.phase === "ready" && (
        <Box paddingX={1}>
          <Input
            placeholder={
              status.type === "busy"
                ? "Agent is working... (Ctrl+C to stop)"
                : "Type a prompt..."
            }
            onSubmit={handleSubmit}
          />
        </Box>
      )}

      {/* Footer shortcuts */}
      <Box paddingX={1}>
        <Text color="gray" dimColor>
          {inputFocused
            ? "[Enter] send  [Esc] unfocus  [Ctrl+C] stop agent"
            : "[i/Enter] focus input  [Ctrl+C] stop  [L] logs  [j/k] scroll  [Esc] back"}
        </Text>
      </Box>
    </Box>
  )
}
