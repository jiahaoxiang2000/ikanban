import { delimiter, isAbsolute, resolve } from "node:path";

import type { WorktreeCleanupPolicy } from "./worktree-manager";

export type AppConfig = {
  opencode: {
    hostname?: string;
    port?: number;
    timeoutMs?: number;
  };
  tasks: {
    maxConcurrent: number;
    cleanupOnSuccess: WorktreeCleanupPolicy;
    cleanupOnFailure: WorktreeCleanupPolicy;
  };
  projects: {
    allowedRootDirectories: string[];
  };
};

export type AppConfigEnv = Record<string, string | undefined>;

export function loadAppConfig(env: AppConfigEnv = process.env): AppConfig {
  const hostname = parseOptionalString(env.IKANBAN_OPENCODE_HOSTNAME);
  const port = parseOptionalPositiveInteger(env.IKANBAN_OPENCODE_PORT, "IKANBAN_OPENCODE_PORT");
  const timeoutMs = parseOptionalPositiveInteger(
    env.IKANBAN_OPENCODE_TIMEOUT_MS,
    "IKANBAN_OPENCODE_TIMEOUT_MS",
  );
  const maxConcurrent = parseOptionalPositiveInteger(
    env.IKANBAN_TASK_MAX_CONCURRENT,
    "IKANBAN_TASK_MAX_CONCURRENT",
    2,
  ) ?? 2;
  const cleanupOnSuccess = parseCleanupPolicy(env.IKANBAN_TASK_CLEANUP_ON_SUCCESS, "keep");
  const cleanupOnFailure = parseCleanupPolicy(env.IKANBAN_TASK_CLEANUP_ON_FAILURE, "keep");
  const allowedRootDirectories = parseAllowedProjectRoots(env.IKANBAN_ALLOWED_PROJECT_PATHS);

  return {
    opencode: {
      hostname,
      port,
      timeoutMs,
    },
    tasks: {
      maxConcurrent,
      cleanupOnSuccess,
      cleanupOnFailure,
    },
    projects: {
      allowedRootDirectories,
    },
  };
}

function parseOptionalString(value: string | undefined): string | undefined {
  if (value === undefined) {
    return undefined;
  }

  const normalized = value.trim();
  return normalized.length > 0 ? normalized : undefined;
}

function parseOptionalPositiveInteger(
  value: string | undefined,
  variable: string,
  fallback?: number,
): number | undefined {
  if (value === undefined || value.trim().length === 0) {
    return fallback;
  }

  const parsed = Number(value);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${variable} must be a positive integer.`);
  }

  return parsed;
}

function parseCleanupPolicy(
  value: string | undefined,
  fallback: WorktreeCleanupPolicy,
): WorktreeCleanupPolicy {
  if (value === undefined || value.trim().length === 0) {
    return fallback;
  }

  const normalized = value.trim().toLowerCase();

  if (normalized !== "keep" && normalized !== "remove") {
    throw new Error("Cleanup policy must be either 'keep' or 'remove'.");
  }

  return normalized;
}

function parseAllowedProjectRoots(value: string | undefined): string[] {
  if (!value || value.trim().length === 0) {
    return [];
  }

  const roots = value
    .split(delimiter)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
    .map((entry) => {
      if (!isAbsolute(entry)) {
        throw new Error(`IKANBAN_ALLOWED_PROJECT_PATHS entries must be absolute paths: ${entry}`);
      }

      return resolve(entry);
    });

  return [...new Set(roots)].sort((left, right) => left.localeCompare(right));
}
