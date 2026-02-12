import { describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

import { ProjectRegistry } from "./project-registry";

function createSandbox(): string {
  return mkdtempSync(join(tmpdir(), "ikanban-project-registry-"));
}

function createRepositoryRoot(parentDirectory: string, directoryName: string): string {
  const repositoryRoot = join(parentDirectory, directoryName);
  mkdirSync(repositoryRoot, { recursive: true });
  mkdirSync(join(repositoryRoot, ".git"));
  return repositoryRoot;
}

describe("ProjectRegistry operations", () => {
  test("adds, lists, selects, and removes projects", async () => {
    const sandbox = createSandbox();

    try {
      const stateFilePath = join(sandbox, "state", "projects.json");
      const projectOneRoot = createRepositoryRoot(sandbox, "repo-one");
      const projectTwoRoot = createRepositoryRoot(sandbox, "repo-two");

      const registry = new ProjectRegistry({ stateFilePath });

      const firstProject = await registry.addProject({
        id: "project-one",
        name: "Project One",
        rootDirectory: projectOneRoot,
      });
      const secondProject = await registry.addProject({
        id: "project-two",
        name: "Project Two",
        rootDirectory: projectTwoRoot,
      });

      const projects = await registry.listProjects();
      expect(projects.map((project) => project.id)).toEqual(["project-one", "project-two"]);
      expect(firstProject.rootDirectory).toBe(resolve(projectOneRoot));
      expect(secondProject.rootDirectory).toBe(resolve(projectTwoRoot));

      expect(await registry.getActiveProjectId()).toBe("project-one");

      await registry.selectProject("project-two");
      expect(await registry.getActiveProjectId()).toBe("project-two");

      const removedSecondProject = await registry.removeProject("project-two");
      expect(removedSecondProject).toBe(true);
      expect(await registry.getActiveProjectId()).toBe("project-one");

      const removedFirstProject = await registry.removeProject("project-one");
      expect(removedFirstProject).toBe(true);
      expect(await registry.getActiveProject()).toBeUndefined();
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });

  test("rejects non-absolute root paths", async () => {
    const sandbox = createSandbox();

    try {
      const stateFilePath = join(sandbox, "state", "projects.json");
      const registry = new ProjectRegistry({ stateFilePath });

      await expect(
        registry.addProject({
          id: "project-relative",
          name: "Relative",
          rootDirectory: "./repo-relative",
        }),
      ).rejects.toThrow("absolute path");
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });

  test("rejects directories that are not repository roots", async () => {
    const sandbox = createSandbox();

    try {
      const stateFilePath = join(sandbox, "state", "projects.json");
      const plainDirectory = join(sandbox, "plain-directory");
      mkdirSync(plainDirectory, { recursive: true });

      const registry = new ProjectRegistry({ stateFilePath });

      await expect(
        registry.addProject({
          id: "project-plain",
          name: "Plain",
          rootDirectory: plainDirectory,
        }),
      ).rejects.toThrow("repository root");
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });

  test("rejects project roots outside allowed directories", async () => {
    const sandbox = createSandbox();

    try {
      const stateFilePath = join(sandbox, "state", "projects.json");
      const outsideRoot = createRepositoryRoot(sandbox, "outside-repo");
      const allowedBase = createRepositoryRoot(sandbox, "allowed-base");

      const registry = new ProjectRegistry({
        stateFilePath,
        allowedRootDirectories: [join(allowedBase, "nested")],
      });

      await expect(
        registry.addProject({
          id: "project-outside",
          name: "Outside",
          rootDirectory: outsideRoot,
        }),
      ).rejects.toThrow("Project rootDirectory is not allowed");
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });

  test("accepts project roots under allowed directories", async () => {
    const sandbox = createSandbox();

    try {
      const stateFilePath = join(sandbox, "state", "projects.json");
      const allowedBase = join(sandbox, "allowed");
      mkdirSync(allowedBase, { recursive: true });
      const repositoryRoot = createRepositoryRoot(allowedBase, "repo-one");

      const registry = new ProjectRegistry({
        stateFilePath,
        allowedRootDirectories: [allowedBase],
      });

      const project = await registry.addProject({
        id: "project-allowed",
        name: "Allowed",
        rootDirectory: repositoryRoot,
      });

      expect(project.rootDirectory).toBe(resolve(repositoryRoot));
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });
});

describe("ProjectRegistry persistence", () => {
  test("persists projects and active selection to JSON state file", async () => {
    const sandbox = createSandbox();

    try {
      const stateFilePath = join(sandbox, "state", "projects.json");
      const projectOneRoot = createRepositoryRoot(sandbox, "repo-one");
      const projectTwoRoot = createRepositoryRoot(sandbox, "repo-two");

      const firstRegistry = new ProjectRegistry({ stateFilePath });
      await firstRegistry.addProject({
        id: "project-one",
        name: "Project One",
        rootDirectory: projectOneRoot,
      });
      await firstRegistry.addProject({
        id: "project-two",
        name: "Project Two",
        rootDirectory: projectTwoRoot,
      });
      await firstRegistry.selectProject("project-two");

      const secondRegistry = new ProjectRegistry({ stateFilePath });
      const restoredProjects = await secondRegistry.listProjects();
      const restoredActiveProject = await secondRegistry.getActiveProject();

      expect(restoredProjects.map((project) => project.id)).toEqual(["project-one", "project-two"]);
      expect(restoredActiveProject?.id).toBe("project-two");
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });
});
