export interface IKanbanProject {
  id: string
  name: string
  path: string
  createdAt: number
}

export type TaskStatus = "Todo" | "InProgress" | "InReview" | "Done"

export const TASK_COLUMNS: readonly TaskStatus[] = [
  "Todo",
  "InProgress",
  "InReview",
  "Done",
] as const

export interface IKanbanTask {
  id: string
  projectId: string
  title: string
  description?: string
  status: TaskStatus
  sessionId?: string
  worktreePath?: string
  branchName?: string
  createdAt: number
}

export type AppView =
  | { kind: "projects" }
  | { kind: "tasks"; projectId: string }
  | { kind: "session"; taskId: string; sessionId: string }

export interface AppState {
  view: AppView
  selectedIndex: number
  columnIndex: number
  showLogs: boolean
  showHelp: boolean
  inputFocused: boolean
  projects: IKanbanProject[]
  tasks: IKanbanTask[]
  /** Last error message displayed to the user */
  lastError: string | null
}

export interface StorageData {
  projects: IKanbanProject[]
  tasks: IKanbanTask[]
}
