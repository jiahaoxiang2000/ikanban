import React, { useState } from "react"
import { Box, Text, useInput } from "ink"
import { store, useStore } from "../state/store.ts"

interface InputProps {
  placeholder?: string
  onSubmit: (value: string) => void
}

export function Input({ placeholder = "Type a prompt…", onSubmit }: InputProps) {
  const { inputFocused } = useStore()
  const [value, setValue] = useState("")

  useInput(
    (input, key) => {
      if (!inputFocused) return

      if (key.return) {
        const trimmed = value.trim()
        if (trimmed) {
          onSubmit(trimmed)
          setValue("")
        }
        return
      }

      if (key.backspace || key.delete) {
        setValue((prev) => prev.slice(0, -1))
        return
      }

      if (key.escape) {
        store.setInputFocused(false)
        return
      }

      // Ignore control sequences
      if (key.ctrl || key.meta) return

      setValue((prev) => prev + input)
    },
    { isActive: inputFocused },
  )

  return (
    <Box
      borderStyle="round"
      borderColor={inputFocused ? "cyan" : "gray"}
      paddingX={1}
    >
      <Text color="cyan" bold>
        {"❯ "}
      </Text>
      {value ? (
        <Text>{value}</Text>
      ) : (
        <Text color="gray" dimColor>
          {placeholder}
        </Text>
      )}
      {inputFocused ? <Text color="cyan">▌</Text> : null}
    </Box>
  )
}
