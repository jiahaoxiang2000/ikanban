import React from "react"
import { Box, Text } from "ink"
import { useStore } from "../state/store.ts"

interface ShortcutSection {
  title: string
  shortcuts: { key: string; action: string }[]
}

const SECTIONS: ShortcutSection[] = [
  {
    title: "Global",
    shortcuts: [
      { key: "?", action: "Toggle this help overlay" },
      { key: "q", action: "Quit application" },
      { key: "Esc", action: "Go back / Cancel" },
    ],
  },
  {
    title: "Navigation",
    shortcuts: [
      { key: "j / Down", action: "Move down / Next item" },
      { key: "k / Up", action: "Move up / Previous item" },
      { key: "h / Left", action: "Move left / Previous column" },
      { key: "l / Right", action: "Move right / Next column" },
      { key: "Enter", action: "Select / Open" },
    ],
  },
  {
    title: "Projects",
    shortcuts: [
      { key: "n", action: "Create new project" },
      { key: "d", action: "Delete selected project" },
      { key: "R", action: "Refresh project/task data" },
    ],
  },
  {
    title: "Tasks",
    shortcuts: [
      { key: "n", action: "Create new task" },
      { key: "e", action: "Edit selected task" },
      { key: "d", action: "Delete selected task" },
      { key: "r", action: "Move task to next column" },
      { key: "R", action: "Refresh project/task data" },
      { key: "Enter", action: "Open session for task" },
    ],
  },
  {
    title: "Session",
    shortcuts: [
      { key: "i / Enter", action: "Focus input" },
      { key: "Ctrl+C", action: "Stop agent" },
      { key: "j / k", action: "Scroll messages" },
      { key: "L", action: "Toggle log panel" },
      { key: "D", action: "Toggle diff view" },
      { key: "T", action: "Toggle todo list" },
      { key: "F", action: "Fork session" },
      { key: "M", action: "Mark task as Done" },
      { key: "S", action: "Stop agent and cleanup" },
      { key: "R", action: "Refresh session state" },
    ],
  },
]

export function HelpOverlay() {
  const { showHelp, view } = useStore()

  if (!showHelp) return null

  // Highlight the section relevant to the current view
  const activeSection =
    view.kind === "projects"
      ? "Projects"
      : view.kind === "tasks"
        ? "Tasks"
        : "Session"

  return (
    <Box
      position="absolute"
      width="100%"
      height="100%"
      justifyContent="center"
      alignItems="center"
      backgroundColor="black"
    >
      <Box
        flexDirection="column"
        width={60}
        borderStyle="double"
        borderColor="cyan"
        paddingX={2}
        paddingY={1}
        backgroundColor="black"
      >
        <Box justifyContent="center" marginBottom={1}>
          <Text bold color="cyan">
            Keyboard Shortcuts
          </Text>
        </Box>

        {SECTIONS.map((section) => {
          const isActive = section.title === activeSection
          return (
            <Box key={section.title} flexDirection="column" marginBottom={1}>
              <Text
                bold
                color={
                  section.title === "Global" || section.title === "Navigation"
                    ? "white"
                    : isActive
                      ? "cyan"
                      : "gray"
                }
              >
                {section.title}
                {isActive ? " (current)" : ""}
              </Text>
              {section.shortcuts.map((s) => (
                <Box key={s.key} gap={1}>
                  <Box width={16}>
                    <Text color="yellow">{s.key}</Text>
                  </Box>
                  <Text color="gray">{s.action}</Text>
                </Box>
              ))}
            </Box>
          )
        })}

        <Box justifyContent="center">
          <Text color="gray" dimColor>
            Press ? to close
          </Text>
        </Box>
      </Box>
    </Box>
  )
}
