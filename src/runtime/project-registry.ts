import { mkdir, stat } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";

import { createProjectRef, type CreateProjectRefInput, type ProjectRef } from "../domain/project";

const REGISTRY_STATE_VERSION = 1;

type ProjectRegistryState = {
  version: number;
  activeProjectId: string | null;
  projects: ProjectRef[];
};

export type ProjectRegistryOptions = {
  stateFilePath: string;
  allowedRootDirectories?: string[];
};

function normalizeAllowedRootDirectories(roots: string[] | undefined): string[] {
  if (!roots || roots.length === 0) {
    return [];
  }

  const normalizedRoots = roots.map((root) => {
    const trimmedRoot = root.trim();

    if (!trimmedRoot) {
      throw new Error("Allowed root directories cannot contain empty values.");
    }

    if (!isAbsolute(trimmedRoot)) {
      throw new Error(`Allowed root directories must be absolute paths: ${trimmedRoot}`);
    }

    return resolve(trimmedRoot);
  });

  return [...new Set(normalizedRoots)].sort((left, right) => left.localeCompare(right));
}

function isWithinDirectory(directory: string, parent: string): boolean {
  const relation = relative(parent, directory);
  return relation === "" || (!relation.startsWith("..") && !isAbsolute(relation));
}

function assertAllowedProjectRoot(rootDirectory: string, allowedRootDirectories: string[]): void {
  if (allowedRootDirectories.length === 0) {
    return;
  }

  const isAllowed = allowedRootDirectories.some((allowedRoot) =>
    isWithinDirectory(rootDirectory, allowedRoot),
  );

  if (!isAllowed) {
    throw new Error(
      `Project rootDirectory is not allowed: ${rootDirectory}. Allowed roots: ${allowedRootDirectories.join(", ")}`,
    );
  }
}

export async function assertAbsoluteRepositoryRoot(rootDirectory: string): Promise<string> {
  const trimmedDirectory = rootDirectory.trim();

  if (trimmedDirectory.length === 0) {
    throw new Error("Project rootDirectory must be a non-empty string.");
  }

  if (!isAbsolute(trimmedDirectory)) {
    throw new Error("Project rootDirectory must be an absolute path.");
  }

  const normalizedDirectory = resolve(trimmedDirectory);
  const directoryStats = await stat(normalizedDirectory).catch(() => undefined);

  if (!directoryStats || !directoryStats.isDirectory()) {
    throw new Error(`Project rootDirectory does not exist as a directory: ${normalizedDirectory}`);
  }

  const gitPath = join(normalizedDirectory, ".git");
  const gitStats = await stat(gitPath).catch(() => undefined);
  if (!gitStats) {
    throw new Error(`Project rootDirectory must be a repository root containing .git: ${normalizedDirectory}`);
  }

  return normalizedDirectory;
}

export class ProjectRegistry {
  private readonly options: ProjectRegistryOptions;
  private readonly allowedRootDirectories: string[];
  private readonly projectsById = new Map<string, ProjectRef>();
  private activeProjectId: string | undefined;
  private loadPromise?: Promise<void>;
  private loaded = false;

  constructor(options: ProjectRegistryOptions) {
    this.options = options;
    this.allowedRootDirectories = normalizeAllowedRootDirectories(options.allowedRootDirectories);
  }

  async addProject(input: CreateProjectRefInput): Promise<ProjectRef> {
    await this.ensureLoaded();

    const projectId = input.id.trim();
    if (this.projectsById.has(projectId)) {
      throw new Error(`Project id already exists: ${projectId}`);
    }

    const normalizedDirectory = await assertAbsoluteRepositoryRoot(input.rootDirectory);
    assertAllowedProjectRoot(normalizedDirectory, this.allowedRootDirectories);
    if (this.findProjectByRoot(normalizedDirectory)) {
      throw new Error(`Project rootDirectory is already registered: ${normalizedDirectory}`);
    }

    const project = createProjectRef({
      ...input,
      id: projectId,
      rootDirectory: normalizedDirectory,
    });

    this.projectsById.set(project.id, project);

    if (!this.activeProjectId) {
      this.activeProjectId = project.id;
    }

    await this.persist();

    return project;
  }

  async removeProject(projectId: string): Promise<boolean> {
    await this.ensureLoaded();

    const normalizedProjectId = projectId.trim();
    const removed = this.projectsById.delete(normalizedProjectId);
    if (!removed) {
      return false;
    }

    if (this.activeProjectId === normalizedProjectId) {
      const nextProject = this.listProjectsSnapshot()[0];
      this.activeProjectId = nextProject?.id;
    }

    await this.persist();

    return true;
  }

  async listProjects(): Promise<ProjectRef[]> {
    await this.ensureLoaded();
    return this.listProjectsSnapshot();
  }

  async selectProject(projectId: string): Promise<ProjectRef> {
    await this.ensureLoaded();

    const normalizedProjectId = projectId.trim();
    const project = this.projectsById.get(normalizedProjectId);
    if (!project) {
      throw new Error(`Unknown project id: ${normalizedProjectId}`);
    }

    this.activeProjectId = project.id;
    await this.persist();

    return project;
  }

  async getProject(projectId: string): Promise<ProjectRef | undefined> {
    await this.ensureLoaded();
    return this.projectsById.get(projectId.trim());
  }

  async getActiveProjectId(): Promise<string | undefined> {
    await this.ensureLoaded();
    return this.activeProjectId;
  }

  async getActiveProject(): Promise<ProjectRef | undefined> {
    await this.ensureLoaded();
    if (!this.activeProjectId) {
      return undefined;
    }

    return this.projectsById.get(this.activeProjectId);
  }

  private listProjectsSnapshot(): ProjectRef[] {
    return [...this.projectsById.values()].sort((left, right) => {
      if (left.createdAt !== right.createdAt) {
        return left.createdAt - right.createdAt;
      }

      return left.id.localeCompare(right.id);
    });
  }

  private async ensureLoaded(): Promise<void> {
    if (this.loaded) {
      return;
    }

    if (!this.loadPromise) {
      this.loadPromise = this.loadState().finally(() => {
        this.loaded = true;
        this.loadPromise = undefined;
      });
    }

    await this.loadPromise;
  }

  private async loadState(): Promise<void> {
    const stateFile = Bun.file(this.options.stateFilePath);
    const exists = await stateFile.exists();

    if (!exists) {
      return;
    }

    const fileContent = await stateFile.text();
    if (!fileContent.trim()) {
      return;
    }

    const parsedState = this.parseState(fileContent);

    for (const project of parsedState.projects) {
      assertAllowedProjectRoot(project.rootDirectory, this.allowedRootDirectories);
      this.projectsById.set(project.id, project);
    }

    const activeProjectId = parsedState.activeProjectId ?? undefined;
    this.activeProjectId = activeProjectId && this.projectsById.has(activeProjectId) ? activeProjectId : undefined;
  }

  private parseState(fileContent: string): ProjectRegistryState {
    const parsedValue = JSON.parse(fileContent) as Partial<ProjectRegistryState>;

    if (!parsedValue || typeof parsedValue !== "object") {
      throw new Error("Invalid project registry state: expected an object.");
    }

    if (parsedValue.version !== REGISTRY_STATE_VERSION) {
      throw new Error(
        `Unsupported project registry state version: ${parsedValue.version ?? "unknown"}.`,
      );
    }

    if (!Array.isArray(parsedValue.projects)) {
      throw new Error("Invalid project registry state: projects must be an array.");
    }

    const projects = parsedValue.projects.map((projectLike) =>
      createProjectRef({
        id: String(projectLike.id),
        name: String(projectLike.name),
        rootDirectory: String(projectLike.rootDirectory),
        createdAt: Number(projectLike.createdAt),
      }),
    );

    const seenProjectIds = new Set<string>();
    const seenProjectRoots = new Set<string>();

    for (const project of projects) {
      if (seenProjectIds.has(project.id)) {
        throw new Error(`Invalid project registry state: duplicate id ${project.id}.`);
      }

      if (seenProjectRoots.has(project.rootDirectory)) {
        throw new Error(
          `Invalid project registry state: duplicate rootDirectory ${project.rootDirectory}.`,
        );
      }

      seenProjectIds.add(project.id);
      seenProjectRoots.add(project.rootDirectory);
    }

    return {
      version: REGISTRY_STATE_VERSION,
      activeProjectId:
        typeof parsedValue.activeProjectId === "string" ? parsedValue.activeProjectId : null,
      projects,
    };
  }

  private async persist(): Promise<void> {
    await mkdir(dirname(this.options.stateFilePath), { recursive: true });

    const state: ProjectRegistryState = {
      version: REGISTRY_STATE_VERSION,
      activeProjectId: this.activeProjectId ?? null,
      projects: this.listProjectsSnapshot(),
    };

    await Bun.write(this.options.stateFilePath, `${JSON.stringify(state, null, 2)}\n`);
  }

  private findProjectByRoot(rootDirectory: string, projects = this.listProjectsSnapshot()): ProjectRef | undefined {
    return projects.find((project) => project.rootDirectory === rootDirectory);
  }
}
