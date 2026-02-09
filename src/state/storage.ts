import { homedir } from "node:os"
import { join } from "node:path"
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs"
import type { StorageData } from "./types.ts"

const DATA_DIR = join(homedir(), ".ikanban")
const DATA_FILE = join(DATA_DIR, "data.json")

const EMPTY_DATA: StorageData = { projects: [], tasks: [] }

export function loadData(): StorageData {
  try {
    if (!existsSync(DATA_FILE)) return { ...EMPTY_DATA, projects: [], tasks: [] }
    const raw = readFileSync(DATA_FILE, "utf-8")
    const parsed = JSON.parse(raw) as Partial<StorageData>
    return {
      projects: Array.isArray(parsed.projects) ? parsed.projects : [],
      tasks: Array.isArray(parsed.tasks) ? parsed.tasks : [],
    }
  } catch {
    return { ...EMPTY_DATA, projects: [], tasks: [] }
  }
}

export function saveData(data: StorageData): void {
  mkdirSync(DATA_DIR, { recursive: true })
  writeFileSync(DATA_FILE, JSON.stringify(data, null, 2), "utf-8")
}
