export type RuntimeLogLevel = "info" | "warn" | "error";

export type RuntimeLogRecord = {
  level: RuntimeLogLevel;
  source: string;
  message: string;
  context?: Record<string, unknown>;
  error?: {
    name: string;
    message: string;
    stack?: string;
  };
};

export type RuntimeLogger = {
  log(record: RuntimeLogRecord): void;
};

export const noopRuntimeLogger: RuntimeLogger = {
  log() {},
};

export function toStructuredError(error: unknown): RuntimeLogRecord["error"] {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack,
    };
  }

  return {
    name: "UnknownError",
    message: formatUnknownError(error),
  };
}

function formatUnknownError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}
