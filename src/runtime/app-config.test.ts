import { describe, expect, test } from "bun:test";
import { delimiter } from "node:path";

import { loadAppConfig } from "./app-config";

describe("loadAppConfig", () => {
  test("returns default values when env is unset", () => {
    const config = loadAppConfig({});

    expect(config).toEqual({
      opencode: {
        hostname: undefined,
        port: undefined,
        timeoutMs: undefined,
      },
      tasks: {
        maxConcurrent: 2,
        cleanupOnSuccess: "keep",
        cleanupOnFailure: "keep",
      },
      projects: {
        allowedRootDirectories: [],
      },
    });
  });

  test("parses opencode, task, and project guardrail settings", () => {
    const config = loadAppConfig({
      IKANBAN_OPENCODE_HOSTNAME: "127.0.0.1",
      IKANBAN_OPENCODE_PORT: "4172",
      IKANBAN_OPENCODE_TIMEOUT_MS: "45000",
      IKANBAN_TASK_MAX_CONCURRENT: "6",
      IKANBAN_TASK_CLEANUP_ON_SUCCESS: "remove",
      IKANBAN_TASK_CLEANUP_ON_FAILURE: "keep",
      IKANBAN_ALLOWED_PROJECT_PATHS: [`/tmp/work`, `/tmp/work/repo-a`, `/tmp/work`].join(delimiter),
    });

    expect(config).toEqual({
      opencode: {
        hostname: "127.0.0.1",
        port: 4172,
        timeoutMs: 45000,
      },
      tasks: {
        maxConcurrent: 6,
        cleanupOnSuccess: "remove",
        cleanupOnFailure: "keep",
      },
      projects: {
        allowedRootDirectories: ["/tmp/work", "/tmp/work/repo-a"],
      },
    });
  });

  test("rejects invalid numeric and cleanup values", () => {
    expect(() => loadAppConfig({ IKANBAN_TASK_MAX_CONCURRENT: "0" })).toThrow(
      "IKANBAN_TASK_MAX_CONCURRENT must be a positive integer.",
    );
    expect(() => loadAppConfig({ IKANBAN_OPENCODE_TIMEOUT_MS: "-1" })).toThrow(
      "IKANBAN_OPENCODE_TIMEOUT_MS must be a positive integer.",
    );
    expect(() => loadAppConfig({ IKANBAN_TASK_CLEANUP_ON_FAILURE: "archive" })).toThrow(
      "Cleanup policy must be either 'keep' or 'remove'.",
    );
  });

  test("rejects non-absolute allowed project paths", () => {
    expect(() =>
      loadAppConfig({
        IKANBAN_ALLOWED_PROJECT_PATHS: ["./repo", "/tmp/repo"].join(delimiter),
      }),
    ).toThrow("IKANBAN_ALLOWED_PROJECT_PATHS entries must be absolute paths");
  });
});
