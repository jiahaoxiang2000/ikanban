import { homedir } from "node:os"
import { join } from "node:path"
import {
  mkdirSync,
  readFileSync,
  writeFileSync,
  existsSync,
} from "node:fs"
import pino from "pino"
import type { StorageData } from "./types.ts"

const DATA_DIR = join(homedir(), ".ikanban")
const DATA_FILE = join(DATA_DIR, "data.json")
const LOG_DIR = join(DATA_DIR, "logs")

function buildRunLogFilePath(): string {
  const time = new Date()
    .toISOString()
    .replace(/:/g, "-")
    .replace(/\..+$/, "")
  const runId = `${time}-pid-${process.pid}`
  return join(LOG_DIR, `${runId}.log`)
}

const RUNTIME_LOG_FILE = buildRunLogFilePath()

const EMPTY_DATA: StorageData = { projects: [], tasks: [] }

export type RuntimeLogLevel = "debug" | "info" | "warn" | "error"

const LOG_LEVEL_ALIASES: Record<string, RuntimeLogLevel> = {
  debug: "debug",
  info: "info",
  warn: "warn",
  warning: "warn",
  error: "error",
}

function resolveLogLevel(): RuntimeLogLevel {
  const raw = process.env.LOG_LEVEL
  if (!raw) return "info"
  const normalized = raw.trim().toLowerCase()
  return LOG_LEVEL_ALIASES[normalized] ?? "info"
}

const ACTIVE_LOG_LEVEL = resolveLogLevel()

const runtimeLogger = pino(
  {
    level: ACTIVE_LOG_LEVEL,
    base: undefined,
    timestamp: pino.stdTimeFunctions.isoTime,
  },
  pino.destination({
    dest: RUNTIME_LOG_FILE,
    mkdir: true,
    sync: false,
  }),
)

export function getActiveLogLevel(): RuntimeLogLevel {
  return ACTIVE_LOG_LEVEL
}

export function getRuntimeLogFilePath(): string {
  return RUNTIME_LOG_FILE
}

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

function safeSerialize(value: unknown): unknown {
  if (value instanceof Error) {
    return {
      name: value.name,
      message: value.message,
      stack: value.stack,
    }
  }
  try {
    return JSON.parse(JSON.stringify(value)) as unknown
  } catch {
    return String(value)
  }
}

function writeRuntimeLog(
  level: RuntimeLogLevel,
  message: string,
  meta?: unknown,
): void {
  const payload = meta === undefined ? undefined : safeSerialize(meta)
  switch (level) {
    case "debug":
      if (payload === undefined) runtimeLogger.debug(message)
      else runtimeLogger.debug(payload, message)
      break
    case "info":
      if (payload === undefined) runtimeLogger.info(message)
      else runtimeLogger.info(payload, message)
      break
    case "warn":
      if (payload === undefined) runtimeLogger.warn(message)
      else runtimeLogger.warn(payload, message)
      break
    case "error":
      if (payload === undefined) runtimeLogger.error(message)
      else runtimeLogger.error(payload, message)
      break
    default:
      runtimeLogger.info(message)
      break
  }
}

export const runtimeLog = {
  debug(message: string, meta?: unknown): void {
    writeRuntimeLog("debug", message, meta)
  },
  info(message: string, meta?: unknown): void {
    writeRuntimeLog("info", message, meta)
  },
  warn(message: string, meta?: unknown): void {
    writeRuntimeLog("warn", message, meta)
  },
  error(message: string, meta?: unknown): void {
    writeRuntimeLog("error", message, meta)
  },
}

export function appendRuntimeLog(
  level: RuntimeLogLevel,
  message: string,
  meta?: unknown,
): void {
  try {
    writeRuntimeLog(level, message, meta)
  } catch {
    // best-effort logging only
  }
}
