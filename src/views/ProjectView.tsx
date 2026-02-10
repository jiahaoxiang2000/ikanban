import React, { useState, useCallback } from "react"
import { Box, Text } from "ink"
import { store, useStore } from "../state/store.ts"
import { useKeyboard, clampIndex } from "../hooks/useKeyboard.ts"
import { Input } from "../components/Input.tsx"
import { gracefulExit } from "../index.tsx"

export function ProjectView() {
  const { projects, selectedIndex, inputFocused } = useStore()
  const [inputMode, setInputMode] = useState<"none" | "new">("none")

  const handleNew = useCallback(() => {
    setInputMode("new")
    store.setInputFocused(true)
  }, [])

  const handleDelete = useCallback(() => {
    const project = projects[selectedIndex]
    if (!project) return
    store.deleteProject(project.id)
    store.setSelectedIndex(clampIndex(selectedIndex, Math.max(0, projects.length - 2)))
  }, [projects, selectedIndex])

  const handleSelect = useCallback(() => {
    const project = projects[selectedIndex]
    if (!project) return
    store.navigate({ kind: "tasks", projectId: project.id })
  }, [projects, selectedIndex])

  const handleSubmit = useCallback(
    (value: string) => {
      if (inputMode === "new") {
        // Format: "name path" or just "name" (uses cwd)
        const parts = value.split(/\s+/)
        const name = parts[0] ?? value
        const path = parts[1] ?? process.cwd()
        store.addProject(name, path)
        setInputMode("none")
        store.setInputFocused(false)
        // Select the newly added project
        store.setSelectedIndex(projects.length)
      }
    },
    [inputMode, projects.length],
  )

  useKeyboard({
    onNavigateUp: () => {
      store.setSelectedIndex(clampIndex(selectedIndex - 1, Math.max(0, projects.length - 1)))
    },
    onNavigateDown: () => {
      store.setSelectedIndex(clampIndex(selectedIndex + 1, Math.max(0, projects.length - 1)))
    },
    onNavigateLeft: () => {},
    onNavigateRight: () => {
      handleSelect()
    },
    onSelect: () => {
      handleSelect()
    },
    onGoBack: () => {
      if (inputMode !== "none") {
        setInputMode("none")
        store.setInputFocused(false)
      }
    },
    onNew: handleNew,
    onDelete: handleDelete,
    onEdit: () => {},
    onMoveTaskRight: () => {},
    onToggleHelp: () => store.toggleHelp(),
    onRefresh: () => store.refresh(),
    onQuit: () => void gracefulExit(0),
  })

  return (
    <Box flexDirection="column" flexGrow={1} padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">
          Projects
        </Text>
        <Text color="gray"> ({projects.length})</Text>
      </Box>

      {projects.length === 0 ? (
        <Box marginBottom={1}>
          <Text color="gray" dimColor>
            No projects yet. Press [n] to create one.
          </Text>
        </Box>
      ) : (
        <Box flexDirection="column" marginBottom={1}>
          {projects.map((project, i) => {
            const isSelected = i === selectedIndex
            return (
              <Box key={project.id} gap={1}>
                <Text color={isSelected ? "cyan" : "gray"}>
                  {isSelected ? ">" : " "}
                </Text>
                <Text bold={isSelected} color={isSelected ? "white" : "gray"}>
                  {project.name}
                </Text>
                <Text color="gray" dimColor>
                  {project.path}
                </Text>
              </Box>
            )
          })}
        </Box>
      )}

      {inputMode === "new" && (
        <Input
          placeholder="project-name /path/to/repo"
          onSubmit={handleSubmit}
        />
      )}

      <Box marginTop={1}>
        <Text color="gray" dimColor>
          [n] new  [d] delete  [Enter/l] open  [R] refresh  [q] quit  [?] help
        </Text>
      </Box>
    </Box>
  )
}
