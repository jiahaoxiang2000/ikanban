import { useEffect, useCallback } from "react"
import { store } from "../state/store"
import { TASK_COLUMNS, type TaskStatus, type AppView } from "../state/types"

interface KeyboardActions {
  onNavigateLeft: () => void
  onNavigateRight: () => void
  onNavigateUp: () => void
  onNavigateDown: () => void
  onSelect: () => void
  onGoBack: () => void
  onNew: () => void
  onDelete: () => void
  onEdit: () => void
  onMoveTaskRight: () => void
  onToggleHelp: () => void
}

// ANSI escape codes for special keys
const ARROW_UP = "\u001b[A"
const ARROW_DOWN = "\u001b[B"
const ARROW_RIGHT = "\u001b[C"
const ARROW_LEFT = "\u001b[D"
const ESCAPE = "\u001b"
const ENTER = "\r"
const BACKSPACE = "\u0008"
const CTRL_C = "\u0003"

export function useKeyboard(actions: KeyboardActions): void {
  const state = store.getState()

  const handleInput = useCallback(
    (input: string, key: { upArrow?: boolean; downArrow?: boolean; leftArrow?: boolean; rightArrow?: boolean; return?: boolean; escape?: boolean; ctrlC?: boolean }) => {
      // Ignore if input is focused (except for global shortcuts)
      if (state.inputFocused) {
        // Allow Escape to blur input
        if (key.escape || input === ESCAPE) {
          store.setInputFocused(false)
        }
        return
      }

      // Handle Ctrl+C globally
      if (key.ctrlC || input === CTRL_C) {
        process.exit(0)
        return
      }

      // Vim-style navigation using arrow keys and letters
      if (key.upArrow || input === "k" || input === "K") {
        actions.onNavigateUp()
      } else if (key.downArrow || input === "j" || input === "J") {
        actions.onNavigateDown()
      } else if (key.leftArrow || input === "h" || input === "H") {
        actions.onNavigateLeft()
      } else if (key.rightArrow || input === "l" || input === "L") {
        actions.onNavigateRight()
      } else if (key.return || input === ENTER) {
        actions.onSelect()
      } else if (key.escape || input === ESCAPE) {
        actions.onGoBack()
      } else if (input === "n" || input === "N") {
        actions.onNew()
      } else if (input === "d" || input === "D") {
        actions.onDelete()
      } else if (input === "e" || input === "E") {
        actions.onEdit()
      } else if (input === "r" || input === "R") {
        actions.onMoveTaskRight()
      } else if (input === "?") {
        actions.onToggleHelp()
      }
    },
    [state.inputFocused, actions]
  )

  useEffect(() => {
    // Import ink's useInput dynamically to avoid SSR issues
    let useInput: (handler: (input: string, key: Record<string, boolean>) => void) => void

    try {
      const ink = require("ink")
      useInput = ink.useInput
    } catch {
      // In test environments or non-ink context, skip
      return
    }

    useInput(handleInput)

    return () => {
      // Cleanup handled by ink
    }
  }, [handleInput])
}

// Helper function to get navigation bounds based on current view
export function getNavigationBounds(
  view: AppView,
  projectsCount: number,
  tasksCount: number
): { maxIndex: number; maxColumn: number } {
  switch (view.kind) {
    case "projects":
      return { maxIndex: Math.max(0, projectsCount - 1), maxColumn: 0 }

    case "tasks":
      return { maxIndex: Math.max(0, tasksCount - 1), maxColumn: TASK_COLUMNS.length - 1 }

    case "session":
      return { maxIndex: 0, maxColumn: 0 }

    default:
      return { maxIndex: 0, maxColumn: 0 }
  }
}

// Helper to clamp index within bounds
export function clampIndex(index: number, maxIndex: number): number {
  return Math.max(0, Math.min(index, maxIndex))
}

// Helper to get next task status for moving right
export function getNextColumnStatus(currentStatus: TaskStatus): TaskStatus | null {
  const currentIndex = TASK_COLUMNS.indexOf(currentStatus)
  const nextStatus = TASK_COLUMNS[currentIndex + 1]
  return nextStatus ?? null
}

// Export keyboard shortcuts reference for help display
export const KEYBOARD_SHORTCUTS = [
  { key: "h", action: "Move left / Previous column" },
  { key: "j", action: "Move down / Next item" },
  { key: "k", action: "Move up / Previous item" },
  { key: "l", action: "Move right / Next column" },
  { key: "Enter", action: "Select / Enter view" },
  { key: "Esc", action: "Go back / Cancel" },
  { key: "n", action: "Create new (project/task)" },
  { key: "d", action: "Delete selected item" },
  { key: "e", action: "Edit selected item" },
  { key: "r", action: "Move task to next column" },
  { key: "?", action: "Show keyboard shortcuts" },
] as const
