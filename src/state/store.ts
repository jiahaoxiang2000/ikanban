import { useSyncExternalStore } from "react"
import type {
  AppState,
  AppView,
  IKanbanProject,
  IKanbanTask,
  TaskStatus,
} from "./types.ts"
import { loadData, saveData } from "./storage.ts"

type Listener = () => void

function createId(): string {
  return crypto.randomUUID()
}

function createStore() {
  const listeners = new Set<Listener>()
  const persisted = loadData()

  let state: AppState = {
    view: { kind: "projects" },
    selectedIndex: 0,
    columnIndex: 0,
    showLogs: false,
    inputFocused: false,
    projects: persisted.projects,
    tasks: persisted.tasks,
  }

  function emit() {
    for (const l of listeners) l()
  }

  function persist() {
    saveData({ projects: state.projects, tasks: state.tasks })
  }

  function setState(partial: Partial<AppState>) {
    state = { ...state, ...partial }
    emit()
  }

  // --- Projects ---

  function addProject(name: string, path: string): IKanbanProject {
    const project: IKanbanProject = {
      id: createId(),
      name,
      path,
      createdAt: Date.now(),
    }
    state = { ...state, projects: [...state.projects, project] }
    persist()
    emit()
    return project
  }

  function deleteProject(id: string) {
    state = {
      ...state,
      projects: state.projects.filter((p) => p.id !== id),
      tasks: state.tasks.filter((t) => t.projectId !== id),
    }
    persist()
    emit()
  }

  function updateProject(
    id: string,
    updates: Partial<Pick<IKanbanProject, "name" | "path">>,
  ) {
    state = {
      ...state,
      projects: state.projects.map((p) =>
        p.id === id ? { ...p, ...updates } : p,
      ),
    }
    persist()
    emit()
  }

  // --- Tasks ---

  function addTask(
    projectId: string,
    title: string,
    description?: string,
  ): IKanbanTask {
    const task: IKanbanTask = {
      id: createId(),
      projectId,
      title,
      description,
      status: "Todo",
      createdAt: Date.now(),
    }
    state = { ...state, tasks: [...state.tasks, task] }
    persist()
    emit()
    return task
  }

  function deleteTask(id: string) {
    state = {
      ...state,
      tasks: state.tasks.filter((t) => t.id !== id),
    }
    persist()
    emit()
  }

  function updateTask(
    id: string,
    updates: Partial<
      Pick<
        IKanbanTask,
        | "title"
        | "description"
        | "status"
        | "sessionId"
        | "worktreePath"
        | "branchName"
      >
    >,
  ) {
    state = {
      ...state,
      tasks: state.tasks.map((t) =>
        t.id === id ? { ...t, ...updates } : t,
      ),
    }
    persist()
    emit()
  }

  function getTasksByProject(projectId: string): IKanbanTask[] {
    return state.tasks.filter((t) => t.projectId === projectId)
  }

  function getTasksByColumn(
    projectId: string,
    status: TaskStatus,
  ): IKanbanTask[] {
    return state.tasks.filter(
      (t) => t.projectId === projectId && t.status === status,
    )
  }

  // --- Navigation ---

  function navigate(view: AppView) {
    setState({ view, selectedIndex: 0, columnIndex: 0, inputFocused: false })
  }

  function setSelectedIndex(index: number) {
    setState({ selectedIndex: index })
  }

  function setColumnIndex(index: number) {
    setState({ columnIndex: index })
  }

  function toggleLogs() {
    setState({ showLogs: !state.showLogs })
  }

  function setInputFocused(focused: boolean) {
    setState({ inputFocused: focused })
  }

  // --- External Store API ---

  function subscribe(listener: Listener) {
    listeners.add(listener)
    return () => {
      listeners.delete(listener)
    }
  }

  function getState(): AppState {
    return state
  }

  return {
    subscribe,
    getState,
    addProject,
    deleteProject,
    updateProject,
    addTask,
    deleteTask,
    updateTask,
    getTasksByProject,
    getTasksByColumn,
    navigate,
    setSelectedIndex,
    setColumnIndex,
    toggleLogs,
    setInputFocused,
  }
}

export const store = createStore()

export function useStore(): AppState {
  return useSyncExternalStore(store.subscribe, store.getState)
}
