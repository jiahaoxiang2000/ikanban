import { Box, Text } from "ink";

import type { RuntimeLogEntry } from "../../runtime/event-bus";

export type LogViewLevel = "info" | "debug";

type LogViewProps = {
  entries: RuntimeLogEntry[];
  level: LogViewLevel;
  scrollOffset: number;
  visibleRows: number;
};

export function LogView({ entries, level, scrollOffset, visibleRows }: LogViewProps) {
  if (entries.length === 0) {
    return (
      <Box flexDirection="column">
        <Text color="cyan">Log view ({level})</Text>
        <Text color="yellow">No log entries yet.</Text>
      </Box>
    );
  }

  const clampedOffset = Math.max(0, Math.min(scrollOffset, Math.max(entries.length - 1, 0)));
  const rows = Math.max(1, visibleRows);
  const end = Math.max(entries.length - clampedOffset, 0);
  const start = Math.max(0, end - rows);
  const visibleEntries = entries.slice(start, end);

  return (
    <Box flexDirection="column">
      <Text color="cyan">
        Log view ({level}) {start + 1}-{end} / {entries.length}
      </Text>
      {visibleEntries.map((entry) => (
        <Box key={`${entry.sequence}:${entry.source}`} flexDirection="column">
          <Text color={entry.level === "error" ? "red" : entry.level === "warn" ? "yellow" : undefined}>
            [{entry.level}] {entry.message}
          </Text>
          {level === "debug" ? (
            <Text color="gray">{safeJson({
              eventType: entry.eventType,
              source: entry.source,
              sequence: entry.sequence,
              emittedAt: entry.emittedAt,
              taskId: entry.taskId,
              projectId: entry.projectId,
              raw: entry.raw,
            })}</Text>
          ) : null}
        </Box>
      ))}
    </Box>
  );
}

function safeJson(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
