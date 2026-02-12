#!/usr/bin/env bun
import { render } from "ink";
import React from "react";
import { App } from "./app.tsx";
import { stopAllAgents, listAgents } from "./agent/registry.ts";
import {
  startRuntime,
  stopRuntime,
  getRuntimeServerUrl,
} from "./agent/runtime.ts";
import { store } from "./state/store.ts";
import {
  appendRuntimeLog,
  getActiveLogLevel,
  getRuntimeLogFilePath,
} from "./state/storage.ts";

function argsToMessage(args: unknown[]): string {
  return args
    .map((value) => {
      if (typeof value === "string") return value;
      if (value instanceof Error) return value.stack ?? value.message;
      try {
        return JSON.stringify(value);
      } catch {
        return String(value);
      }
    })
    .join(" ");
}

function installConsolePersistence() {
  const originalDebug = console.debug.bind(console);
  const originalLog = console.log.bind(console);
  const originalWarn = console.warn.bind(console);
  const originalError = console.error.bind(console);

  console.debug = (...args: unknown[]) => {
    appendRuntimeLog("debug", argsToMessage(args), { source: "console.debug" });
    originalDebug(...args);
  };

  console.log = (...args: unknown[]) => {
    appendRuntimeLog("info", argsToMessage(args), { source: "console.log" });
    originalLog(...args);
  };

  console.warn = (...args: unknown[]) => {
    appendRuntimeLog("warn", argsToMessage(args), { source: "console.warn" });
    originalWarn(...args);
  };

  console.error = (...args: unknown[]) => {
    appendRuntimeLog("error", argsToMessage(args), { source: "console.error" });
    originalError(...args);
  };
}

installConsolePersistence();

appendRuntimeLog("info", "Runtime logging initialized", {
  source: "index.bootstrap",
  logLevel: getActiveLogLevel(),
  logFile: getRuntimeLogFilePath(),
});

try {
  await startRuntime();
} catch (err) {
  const message = err instanceof Error ? err.message : String(err);
  appendRuntimeLog("error", `Failed to start shared runtime: ${message}`, {
    source: "runtime.start",
    err,
  });
  store.setLastError(`Failed to start opencode runtime: ${message}`);
}

const instance = render(<App />);

let cleaningUp = false;

async function cleanup() {
  if (cleaningUp) return;
  cleaningUp = true;

  const agents = listAgents();
  if (agents.length > 0) {
    // Give agents a grace period to shut down
    // Race against a timeout so we don't hang forever
    await Promise.race([
      stopAllAgents(),
      new Promise((resolve) => setTimeout(resolve, 5000)),
    ]);
  }

  await stopRuntime();

  instance.unmount();
}

/** Exported so views can call it for the `q` key quit path */
export async function gracefulExit(code = 0): Promise<void> {
  await cleanup();
  process.exit(code);
}

process.on("SIGINT", () => {
  appendRuntimeLog("info", "Received SIGINT, cleaning up", {
    source: "process.SIGINT",
  });
  void cleanup().then(() => process.exit(0));
});

process.on("SIGTERM", () => {
  appendRuntimeLog("info", "Received SIGTERM, cleaning up", {
    source: "process.SIGTERM",
  });
  void cleanup().then(() => process.exit(0));
});

// Handle uncaught errors gracefully
process.on("uncaughtException", (err) => {
  appendRuntimeLog("error", `Uncaught error: ${err.message}`, {
    source: "process.uncaughtException",
    error: err,
  });
  store.setLastError(`Uncaught error: ${err.message}`);
  // Don't exit – let the user see the error and decide
});

process.on("unhandledRejection", (reason) => {
  const message = reason instanceof Error ? reason.message : String(reason);
  appendRuntimeLog("error", `Unhandled rejection: ${message}`, {
    source: "process.unhandledRejection",
    reason,
  });
  store.setLastError(`Unhandled rejection: ${message}`);
});
