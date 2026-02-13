import { Box, Text } from "ink";
import React, { useMemo } from "react";
import { VirtualList } from "ink-virtual-list";

import type { RuntimeLogEntry } from "../../runtime/event-bus";

export type LogViewLevel = "info" | "debug";

type LogViewProps = {
  entries: RuntimeLogEntry[];
  level: LogViewLevel;
  scrollOffset: number;
  visibleRows: number;
};

function safeJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

// Compact single-line JSON for VirtualList
function compactJson(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

interface LogRowProps {
  entry: RuntimeLogEntry;
  showDebugDetails: boolean;
}

// Extract context from raw if it exists
function getRawContext(raw: unknown): unknown {
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if ("context" in obj) {
      return obj.context;
    }
  }
  return raw;
}

const LogRow = React.memo(function LogRow({ entry, showDebugDetails }: LogRowProps) {
  const messageColor = entry.level === "error" ? "red" : entry.level === "warn" ? "yellow" : entry.level === "debug" ? "gray" : undefined;
  
  // Get the raw context (which contains the actual event data)
  const rawContext = getRawContext(entry.raw);
  const hasRawContent = Boolean(rawContext && (typeof rawContext === "object" ? Object.keys(rawContext).length > 0 : rawContext));
  
  return (
    <Box flexDirection="column">
      <Text color={messageColor}>
        [{entry.level}] {entry.message}
      </Text>
      {showDebugDetails && hasRawContent && (
        <Text color="gray">{compactJson(rawContext)}</Text>
      )}
    </Box>
  );
});

export function LogView({ entries, level, scrollOffset, visibleRows }: LogViewProps) {
  const filteredEntries = useMemo(() => {
    return level === "debug" ? entries : entries.filter((entry) => entry.level !== "debug");
  }, [entries, level]);

  if (entries.length === 0) {
    return (
      <Box flexDirection="column">
        <Text color="cyan">Log view ({level})</Text>
        <Text color="yellow">No log entries yet.</Text>
      </Box>
    );
  }

  const showDebugDetails = level === "debug";
  const listHeight = Math.max(1, visibleRows - 1); // Reserve 1 line for header

  // Convert scrollOffset (0=latest) to selectedIndex (0=first item in array)
  const selectedIndex = Math.max(0, Math.min(
    filteredEntries.length - 1,
    filteredEntries.length - 1 - scrollOffset
  ));

  // Always use VirtualList - debug info is now compact single-line JSON
  return (
    <Box flexDirection="column">
      <Text color="cyan">
        Log view ({level}) {filteredEntries.length} entries (offset: {scrollOffset})
      </Text>
      <VirtualList
        items={filteredEntries}
        height={listHeight}
        itemHeight={1}
        selectedIndex={selectedIndex}
        keyExtractor={(entry, index) => `${entry.sequence}:${entry.source}:${index}`}
        renderItem={({ item, isSelected }) => (
          <LogRow
            entry={item}
            showDebugDetails={showDebugDetails}
          />
        )}
      />
    </Box>
  );
}
