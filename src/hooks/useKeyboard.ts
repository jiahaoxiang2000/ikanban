import { useInput } from "ink"
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

export function useKeyboard(actions: KeyboardActions): void {
  useInput((input, key) => {
    const state = store.getState()

    // Ignore if input is focused (except Escape to blur)
    if (state.inputFocused) {
      if (key.escape) {
        store.setInputFocused(false)
      }
      return
    }

    // Vim-style navigation
    if (key.upArrow || input === "k") {
      actions.onNavigateUp()
    } else if (key.downArrow || input === "j") {
      actions.onNavigateDown()
    } else if (key.leftArrow || input === "h") {
      actions.onNavigateLeft()
    } else if (key.rightArrow || input === "l") {
      actions.onNavigateRight()
    } else if (key.return) {
      actions.onSelect()
    } else if (key.escape) {
      actions.onGoBack()
    } else if (input === "n") {
      actions.onNew()
    } else if (input === "d") {
      actions.onDelete()
    } else if (input === "e") {
      actions.onEdit()
    } else if (input === "r") {
      actions.onMoveTaskRight()
    } else if (input === "?") {
      actions.onToggleHelp()
    }
  })
}

// Helper function to get navigation bounds based on current view
export function getNavigationBounds(
  view: AppView,
  projectsCount: number,
  tasksCount: number,
): { maxIndex: number; maxColumn: number } {
  switch (view.kind) {
    case "projects":
      return { maxIndex: Math.max(0, projectsCount - 1), maxColumn: 0 }

    case "tasks":
      return {
        maxIndex: Math.max(0, tasksCount - 1),
        maxColumn: TASK_COLUMNS.length - 1,
      }

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
export function getNextColumnStatus(
  currentStatus: TaskStatus,
): TaskStatus | null {
  const currentIndex = TASK_COLUMNS.indexOf(currentStatus)
  const nextStatus = TASK_COLUMNS[currentIndex + 1]
  return nextStatus ?? null
}

// Export keyboard shortcuts reference for help display
export const KEYBOARD_SHORTCUTS = [
  { key: "h", action: "Move left / Previous column" },
  { key: "j", action: "Move down / Next item" },
  { key: "k", action: "Move up / Previous item" },
  { key: "l", action: "Move right / Next column / Open" },
  { key: "Enter", action: "Select / Open" },
  { key: "Esc", action: "Go back / Cancel" },
  { key: "n", action: "Create new (project/task)" },
  { key: "d", action: "Delete selected item" },
  { key: "e", action: "Edit selected item" },
  { key: "r", action: "Move task to next column" },
  { key: "?", action: "Show keyboard shortcuts" },
] as const
