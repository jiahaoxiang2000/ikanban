import React, { useState, useEffect, useRef } from "react"
import { Box, Text } from "ink"
import { useStore } from "../state/store.ts"
import { getAgent } from "../agent/registry.ts"

interface LogEntry {
  timestamp: number
  kind: "stdout" | "event"
  text: string
}

export function LogPanel() {
  const { view, showLogs } = useStore()
  const [logs, setLogs] = useState<LogEntry[]>([])
  const abortRef = useRef<AbortController | null>(null)

  useEffect(() => {
    // Clean up previous subscription
    abortRef.current?.abort()
    abortRef.current = null
    setLogs([])

    if (view.kind !== "session") return

    const agent = getAgent(view.taskId)
    if (!agent) return

    function pushLog(kind: LogEntry["kind"], text: string) {
      setLogs((prev: LogEntry[]) => [
        ...prev.slice(-200), // keep last 200 entries
        { timestamp: Date.now(), kind, text },
      ])
    }

    const controller = new AbortController()
    abortRef.current = controller

    // Subscribe to global events via the SDK client's SSE endpoint
    void (async () => {
      try {
        const result = await agent.client.event.subscribe()

        if (controller.signal.aborted) return

        for await (const event of result.stream) {
          if (controller.signal.aborted) break
          const summary =
            typeof event === "object" && event !== null
              ? JSON.stringify(event).slice(0, 200)
              : String(event)
          pushLog("event", summary)
        }
      } catch (err: unknown) {
        if (!controller.signal.aborted) {
          pushLog("event", `[error] ${String(err)}`)
        }
      }
    })()

    return () => {
      controller.abort()
    }
  }, [view.kind === "session" ? view.taskId : null])

  if (!showLogs) return null

  return (
    <Box
      flexDirection="column"
      width={50}
      borderStyle="single"
      borderColor="gray"
      paddingX={1}
      flexShrink={0}
    >
      <Box justifyContent="center" marginBottom={1}>
        <Text bold color="magenta">
          Logs
        </Text>
      </Box>

      {logs.length === 0 ? (
        <Text color="gray" dimColor>
          No log entries yet.
        </Text>
      ) : (
        <Box flexDirection="column" overflow="hidden">
          {logs.slice(-20).map((entry: LogEntry, i: number) => {
            const time = new Date(entry.timestamp)
              .toLocaleTimeString("en-US", { hour12: false })
            const color = entry.kind === "stdout" ? "white" : "cyan"
            return (
              <Text key={i} color={color} wrap="truncate">
                <Text color="gray">{time}</Text> {entry.text}
              </Text>
            )
          })}
        </Box>
      )}
    </Box>
  )
}
