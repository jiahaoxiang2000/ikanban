import { describe, expect, test } from "bun:test";

import { OpenCodeRuntime } from "./opencode-runtime";

type FakeRuntime = {
  server: {
    url: string;
    closeCalls: number;
    close(): void;
  };
  client: {
    id: string;
  };
};

function buildRuntime(url: string, clientId: string): FakeRuntime {
  return {
    server: {
      url,
      closeCalls: 0,
      close() {
        this.closeCalls += 1;
      },
    },
    client: {
      id: clientId,
    },
  };
}

describe("OpenCodeRuntime lifecycle", () => {
  test("starts server once and reuses the same instance", async () => {
    const startCalls: string[] = [];
    const runtimeState = buildRuntime("http://localhost:9000", "root-client");

    const runtime = new OpenCodeRuntime(
      {},
      {
        createOpencode: async () => {
          startCalls.push("start");
          return runtimeState as never;
        },
      },
    );

    const firstStart = await runtime.start();
    const secondStart = await runtime.start();

    expect(firstStart).toBe(secondStart);
    expect(startCalls.length).toBe(1);
    expect(runtime.isRunning()).toBe(true);
  });

  test("stop closes active server and allows restart", async () => {
    const createdRuntimes = [
      buildRuntime("http://localhost:9000", "root-client-1"),
      buildRuntime("http://localhost:9001", "root-client-2"),
    ];

    let nextRuntimeIndex = 0;

    const runtime = new OpenCodeRuntime(
      {},
      {
        createOpencode: async () => createdRuntimes[nextRuntimeIndex++] as never,
      },
    );

    await runtime.start();
    await runtime.stop();

    expect(createdRuntimes[0]!.server.closeCalls).toBe(1);
    expect(runtime.isRunning()).toBe(false);

    const restarted = await runtime.start();
    expect(restarted.server.url).toBe("http://localhost:9001");
  });

  test("restart replaces server instance", async () => {
    const createdRuntimes = [
      buildRuntime("http://localhost:9000", "root-client-1"),
      buildRuntime("http://localhost:9001", "root-client-2"),
    ];
    let nextRuntimeIndex = 0;

    const runtime = new OpenCodeRuntime(
      {},
      {
        createOpencode: async () => createdRuntimes[nextRuntimeIndex++] as never,
      },
    );

    const first = await runtime.start();
    const second = await runtime.restart();

    expect(first.server.url).toBe("http://localhost:9000");
    expect(second.server.url).toBe("http://localhost:9001");
    expect(createdRuntimes[0]!.server.closeCalls).toBe(1);
  });

  test("writes structured error logs when startup fails", async () => {
    const logs: Array<{ level: string; source: string; message: string }> = [];

    const runtime = new OpenCodeRuntime(
      {
        logger: {
          log(record) {
            logs.push({
              level: record.level,
              source: record.source,
              message: record.message,
            });
          },
        },
      },
      {
        createOpencode: async () => {
          throw new Error("cannot start");
        },
      },
    );

    await expect(runtime.start()).rejects.toThrow("cannot start");
    expect(logs).toEqual([
      {
        level: "error",
        source: "opencode-runtime.start",
        message: "Failed to start OpenCode runtime.",
      },
    ]);
  });
});

describe("OpenCodeRuntime scoped clients", () => {
  test("reuses client per normalized directory", async () => {
    const runtimeState = buildRuntime("http://localhost:9000", "root-client");
    const createClientCalls: Array<{ baseUrl?: string; directory?: string }> = [];

    const runtime = new OpenCodeRuntime(
      {},
      {
        createOpencode: async () => runtimeState as never,
        createOpencodeClient: (config) => {
          createClientCalls.push({
            baseUrl: config?.baseUrl,
            directory: config?.directory,
          });

          return {
            config,
          } as never;
        },
      },
    );

    const clientA = await runtime.getClient("/tmp/demo/project");
    const clientB = await runtime.getClient("/tmp/demo/project/");
    const clientC = await runtime.getClient("/tmp/demo/other");

    expect(clientA).toBe(clientB);
    expect(clientA).not.toBe(clientC);
    expect(createClientCalls.length).toBe(2);
    expect(createClientCalls[0]).toEqual({
      baseUrl: "http://localhost:9000",
      directory: "/tmp/demo/project",
    });
  });

  test("clears scoped client cache after stop", async () => {
    const runtimeState = buildRuntime("http://localhost:9000", "root-client");
    let createClientCount = 0;

    const runtime = new OpenCodeRuntime(
      {},
      {
        createOpencode: async () => runtimeState as never,
        createOpencodeClient: () => {
          createClientCount += 1;
          return { id: createClientCount } as never;
        },
      },
    );

    const first = await runtime.getClient("/tmp/demo/project");
    await runtime.stop();
    const second = await runtime.getClient("/tmp/demo/project");

    expect(first).not.toBe(second);
    expect(createClientCount).toBe(2);
  });
});
