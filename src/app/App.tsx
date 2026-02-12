import { basename, resolve } from "node:path";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Box, Text, useApp, useInput, useStdout } from "ink";

import type { ProjectRef } from "../domain/project";
import type { TaskRuntime } from "../domain/task";
import type { RuntimeEventMap, RuntimeLogEntry } from "../runtime/event-bus";
import { ProjectRegistry } from "../runtime/project-registry";
import { RuntimeEventBus } from "../runtime/event-bus";
import { OpenCodeRuntime } from "../runtime/opencode-runtime";
import {
  TaskOrchestrator,
  type RunTaskInput,
  type TaskOrchestratorEvent,
} from "../runtime/task-orchestrator";
import { WorktreeManager } from "../runtime/worktree-manager";
import { LogView, type LogViewLevel } from "./views/log-view";
import { ProjectSelectorView } from "./views/project-selector-view";
import { TaskBoardView } from "./views/task-board-view";
import { nextRoute, ROUTE_DESCRIPTORS, type AppRoute } from "./routes";

type BannerTone = "info" | "success" | "warn" | "error";

type StatusBanner = {
  tone: BannerTone;
  message: string;
  at: number;
};

type TaskSessionMessage = {
  messageID: string;
  role: string;
  preview: string;
  createdAt: number;
};

type PromptModel = NonNullable<RunTaskInput["model"]>;

type ModelOption = {
  label: string;
  model?: PromptModel;
};

export type AppServices = {
  runtime: OpenCodeRuntime;
  projectRegistry: ProjectRegistry;
  orchestrator: TaskOrchestrator;
  worktreeManager: WorktreeManager;
  eventBus: RuntimeEventBus;
};

type AppProps = {
  services: AppServices;
  defaultProjectDirectory?: string;
  initialRoute?: AppRoute;
};

const MAX_LOG_ENTRIES = 200;
const LOG_SCROLL_STEP = 1;
const LOG_SCROLL_PAGE = 8;

export function App({
  services,
  defaultProjectDirectory,
  initialRoute = "project-selector",
}: AppProps) {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const [loading, setLoading] = useState(true);
  const [busyMessage, setBusyMessage] = useState<string>();
  const [errorMessage, setErrorMessage] = useState<string>();
  const [statusBanner, setStatusBanner] = useState<StatusBanner>();
  const [route, setRoute] = useState<AppRoute>(initialRoute);
  const [projects, setProjects] = useState<ProjectRef[]>([]);
  const [activeProjectId, setActiveProjectId] = useState<string>();
  const [selectedProjectIndex, setSelectedProjectIndex] = useState(0);
  const [tasks, setTasks] = useState<TaskRuntime[]>([]);
  const [selectedTaskIndex, setSelectedTaskIndex] = useState(0);
  const [logs, setLogs] = useState<RuntimeLogEntry[]>([]);
  const [sessionMessagesByTaskID, setSessionMessagesByTaskID] = useState<
    Record<string, TaskSessionMessage[]>
  >({});
  const [newProjectPathInput, setNewProjectPathInput] = useState<string>();
  const [newTaskPromptInput, setNewTaskPromptInput] = useState<string>();
  const [taskModel, setTaskModel] = useState<PromptModel | undefined>();
  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  const [modelFilterInput, setModelFilterInput] = useState("");
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [selectedModelOptionIndex, setSelectedModelOptionIndex] = useState(0);
  const [promptByTaskID, setPromptByTaskID] = useState<Record<string, string>>(
    {},
  );
  const [modelByTaskID, setModelByTaskID] = useState<
    Record<string, PromptModel | undefined>
  >({});
  const [followUpPromptInput, setFollowUpPromptInput] = useState<string>();
  const [logViewLevel, setLogViewLevel] = useState<LogViewLevel>("info");
  const [isLogViewOpen, setIsLogViewOpen] = useState(false);
  const [logScrollOffset, setLogScrollOffset] = useState(0);

  const pushBanner = useCallback((tone: BannerTone, message: string) => {
    setStatusBanner({
      tone,
      message,
      at: Date.now(),
    });
  }, []);

  const refreshProjects = useCallback(async () => {
    const nextProjects = await services.projectRegistry.listProjects();
    const activeProject = await services.projectRegistry.getActiveProject();

    setProjects(nextProjects);
    setActiveProjectId(activeProject?.id);

    if (nextProjects.length === 0) {
      setSelectedProjectIndex(0);
      return;
    }

    const selectedIndex = activeProject
      ? nextProjects.findIndex((project) => project.id === activeProject.id)
      : 0;

    setSelectedProjectIndex(selectedIndex >= 0 ? selectedIndex : 0);
  }, [services.projectRegistry]);

  const activeProject = useMemo(
    () => projects.find((project) => project.id === activeProjectId),
    [projects, activeProjectId],
  );

  const tasksForActiveProject = useMemo(() => {
    if (!activeProject) {
      return [];
    }

    return tasks.filter((task) => task.projectId === activeProject.id);
  }, [activeProject, tasks]);

  const selectedTask = tasksForActiveProject[selectedTaskIndex];

  const taskLogs = useMemo(() => {
    if (!selectedTask) {
      return logs;
    }

    const scoped = logs.filter(
      (entry) => !entry.taskId || entry.taskId === selectedTask.taskId,
    );
    return scoped;
  }, [logs, selectedTask]);

  const taskMessages = useMemo(() => {
    if (!selectedTask) {
      return [];
    }

    return sessionMessagesByTaskID[selectedTask.taskId] ?? [];
  }, [selectedTask, sessionMessagesByTaskID]);

  const filteredModelOptions = useMemo(() => {
    return filterModelOptions(modelOptions, modelFilterInput);
  }, [modelOptions, modelFilterInput]);

  useEffect(() => {
    if (!modelPickerOpen) {
      return;
    }

    setSelectedModelOptionIndex((current) => {
      if (filteredModelOptions.length === 0) {
        return 0;
      }

      return Math.max(0, Math.min(current, filteredModelOptions.length - 1));
    });
  }, [filteredModelOptions, modelPickerOpen]);

  useEffect(() => {
    setSelectedProjectIndex((current) => {
      if (projects.length === 0) {
        return 0;
      }

      return Math.max(0, Math.min(current, projects.length - 1));
    });
  }, [projects]);

  useEffect(() => {
    setSelectedTaskIndex((current) => {
      if (tasksForActiveProject.length === 0) {
        return 0;
      }

      return Math.max(0, Math.min(current, tasksForActiveProject.length - 1));
    });
  }, [tasksForActiveProject]);

  useEffect(() => {
    let cancelled = false;

    const boot = async () => {
      setLoading(true);
      setErrorMessage(undefined);

      try {
        await services.runtime.start();
        await services.orchestrator.initialize();
        await ensureDefaultProject(
          services.projectRegistry,
          defaultProjectDirectory,
        );

        if (cancelled) {
          return;
        }

        await refreshProjects();
        if (cancelled) {
          return;
        }

        setTasks(services.orchestrator.listTasks());
        pushBanner("success", "Runtime ready. Use Tab to switch views.");
      } catch (error) {
        if (cancelled) {
          return;
        }

        setErrorMessage(toErrorMessage(error));
        pushBanner("error", "Failed to initialize runtime.");
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void boot();

    return () => {
      cancelled = true;
      void services.runtime.stop();
    };
  }, [services, defaultProjectDirectory, refreshProjects, pushBanner]);

  useEffect(() => {
    const unsubscribe = services.orchestrator.subscribe((event) => {
      relayOrchestratorEvent(event, services.eventBus, (taskID) => {
        return services.orchestrator.getTask(taskID)?.projectId ?? "pending";
      });
      setTasks(services.orchestrator.listTasks());
    });

    return unsubscribe;
  }, [services.orchestrator, services.eventBus]);

  useEffect(() => {
    const unsubscribeLogs = services.eventBus.subscribeToLogs((entry) => {
      setLogs((current) => {
        const next = [...current, entry];
        return next.slice(-MAX_LOG_ENTRIES);
      });
    });

    const unsubscribeEvents = services.eventBus.subscribe((event) => {
      if (event.type !== "session.message.received") {
        return;
      }

      const payload =
        event.payload as RuntimeEventMap["session.message.received"];

      setSessionMessagesByTaskID((current) => {
        const existing = current[payload.taskId] ?? [];
        const nextMessage = {
          messageID: payload.messageID,
          role: payload.role,
          preview: payload.preview,
          createdAt: payload.createdAt,
        };
        const existingIndex = existing.findIndex(
          (message) => message.messageID === payload.messageID,
        );

        if (existingIndex >= 0) {
          const previous = existing[existingIndex];
          if (!previous) {
            return current;
          }

          if (
            previous.role === nextMessage.role &&
            previous.preview === nextMessage.preview &&
            previous.createdAt === nextMessage.createdAt
          ) {
            return current;
          }

          const updated = [...existing];
          updated[existingIndex] = nextMessage;
          return {
            ...current,
            [payload.taskId]: updated.slice(-20),
          };
        }

        const nextMessages = [
          ...existing,
          nextMessage,
        ].slice(-20);

        return {
          ...current,
          [payload.taskId]: nextMessages,
        };
      });
    });

    const unsubscribeUpdates = services.eventBus.subscribeToUiUpdates(
      (update) => {
        const text = `${update.scope}.${update.action} (${update.taskId})`;
        if (update.eventType === "task.failed") {
          pushBanner("error", text);
          return;
        }

        pushBanner("info", text);
      },
    );

    return () => {
      unsubscribeLogs();
      unsubscribeEvents();
      unsubscribeUpdates();
    };
  }, [services.eventBus, pushBanner]);

  const selectProject = useCallback(
    async (projectID: string) => {
      setBusyMessage("Selecting project...");
      try {
        const project = await services.projectRegistry.selectProject(projectID);
        setActiveProjectId(project.id);
        pushBanner("success", `Active project: ${project.name}`);
        setRoute("task-board");
      } catch (error) {
        setErrorMessage(toErrorMessage(error));
      } finally {
        setBusyMessage(undefined);
      }
    },
    [services.projectRegistry, pushBanner],
  );

  const createProject = useCallback(
    async (inputPath: string) => {
      const normalizedInputPath = inputPath.trim();
      if (!normalizedInputPath) {
        pushBanner("warn", "Project path is required.");
        return;
      }

      const rootDirectory = resolve(normalizedInputPath);
      const existingProject = projects.find(
        (project) => project.rootDirectory === rootDirectory,
      );

      setBusyMessage("Creating project...");
      try {
        if (existingProject) {
          pushBanner(
            "warn",
            `Project already exists for ${rootDirectory}. New project was not created.`,
          );
          return;
        }

        const projectName = basename(rootDirectory) || "project";
        const projectID = buildUniqueProjectID(
          projectName,
          projects.map((project) => project.id),
        );
        const project = await services.projectRegistry.addProject({
          id: projectID,
          name: projectName,
          rootDirectory,
        });

        await services.projectRegistry.selectProject(project.id);
        await refreshProjects();
        setRoute("task-board");
        pushBanner("success", `Created project: ${project.name}`);
      } catch (error) {
        pushBanner("error", toErrorMessage(error));
      } finally {
        setBusyMessage(undefined);
      }
    },
    [projects, services.projectRegistry, refreshProjects, pushBanner],
  );

  const startProjectCreationInput = useCallback(() => {
    const defaultPath = resolve(defaultProjectDirectory ?? process.cwd());
    setNewProjectPathInput(defaultPath);
    pushBanner("info", "Enter project path and press Enter to create project.");
  }, [defaultProjectDirectory, pushBanner]);

  const runTask = useCallback(
    async (
      initialPrompt?: string,
      model: PromptModel | undefined = taskModel,
    ) => {
      if (!activeProject) {
        pushBanner("warn", "No active project selected.");
        return;
      }

      const prompt = initialPrompt?.trim() || "";
      const taskID = createTaskID(activeProject.id, prompt);
      const resolvedPrompt =
        prompt || buildDefaultPrompt(activeProject.name, taskID);
      setPromptByTaskID((current) => ({
        ...current,
        [taskID]: resolvedPrompt,
      }));
      setModelByTaskID((current) => ({
        ...current,
        [taskID]: model,
      }));

      setBusyMessage(`Running ${taskID}...`);
      try {
        await services.orchestrator.runTask({
          taskId: taskID,
          projectId: activeProject.id,
          initialPrompt: resolvedPrompt,
          title: `Task ${taskID}`,
          model,
        });
        pushBanner("success", `Task ${taskID} finished.`);
      } catch (error) {
        pushBanner("error", toErrorMessage(error));
      } finally {
        setBusyMessage(undefined);
        setTasks(services.orchestrator.listTasks());
      }
    },
    [activeProject, pushBanner, services.orchestrator, taskModel],
  );

  const sendFollowUpPrompt = useCallback(
    async (prompt: string) => {
      const task = selectedTask;
      if (!task) {
        pushBanner("warn", "No task selected.");
        return;
      }

      if (task.state !== "review") {
        pushBanner(
          "warn",
          "Task must be in review state to send a follow-up prompt.",
        );
        return;
      }

      setBusyMessage(`Sending follow-up to ${task.taskId}...`);
      try {
        await services.orchestrator.sendFollowUpPrompt(task.taskId, prompt);
        pushBanner(
          "success",
          `Follow-up completed for ${task.taskId}. Now in review.`,
        );
      } catch (error) {
        pushBanner("error", toErrorMessage(error));
      } finally {
        setBusyMessage(undefined);
        setTasks(services.orchestrator.listTasks());
      }
    },
    [selectedTask, pushBanner, services.orchestrator],
  );

  const mergeSelectedTask = useCallback(async () => {
    const task = selectedTask;
    if (!task) {
      pushBanner("warn", "No task selected.");
      return;
    }

    if (task.state !== "review") {
      pushBanner("warn", "Task must be in review state to merge.");
      return;
    }

    setBusyMessage(`Merging ${task.taskId}...`);
    try {
      const result = await services.orchestrator.mergeTask(task.taskId);
      pushBanner(
        "success",
        `Merged branch ${result.branch} for task ${task.taskId}.`,
      );
    } catch (error) {
      pushBanner("error", toErrorMessage(error));
    } finally {
      setBusyMessage(undefined);
      setTasks(services.orchestrator.listTasks());
    }
  }, [selectedTask, pushBanner, services.orchestrator]);

  const startFollowUpPromptInput = useCallback(() => {
    const task = selectedTask;
    if (!task) {
      pushBanner("warn", "No task selected.");
      return;
    }

    if (task.state !== "review") {
      pushBanner(
        "warn",
        "Task must be in review state to send a follow-up prompt.",
      );
      return;
    }

    setFollowUpPromptInput("");
    pushBanner("info", "Enter follow-up prompt and press Enter to send.");
  }, [selectedTask, pushBanner]);

  const startTaskPromptInput = useCallback(() => {
    if (!activeProject) {
      pushBanner("warn", "No active project selected.");
      return;
    }

    setNewTaskPromptInput("");
    pushBanner(
      "info",
      `Enter the first task prompt and press Enter to run (model: ${formatModel(taskModel)}).`,
    );
  }, [activeProject, pushBanner, taskModel]);

  const openTaskModelPicker = useCallback(async () => {
    if (!activeProject) {
      pushBanner("warn", "No active project selected.");
      return;
    }

    setBusyMessage("Loading available models...");
    try {
      const options = await loadModelOptions(
        services,
        activeProject.rootDirectory,
      );
      setModelOptions(options);
      setModelFilterInput("");
      const activeIndex = options.findIndex((candidate) =>
        isSameModel(candidate.model, taskModel),
      );
      setSelectedModelOptionIndex(activeIndex >= 0 ? activeIndex : 0);
      setModelPickerOpen(true);
      pushBanner(
        "info",
        "Type to filter models, use arrows, Enter select, Esc cancel.",
      );
    } catch (error) {
      pushBanner("error", toErrorMessage(error));
    } finally {
      setBusyMessage(undefined);
    }
  }, [activeProject, pushBanner, services, taskModel]);

  const toggleLogViewLevel = useCallback(() => {
    setLogViewLevel((current) => {
      const next = current === "info" ? "debug" : "info";
      pushBanner("info", `Log view level set to ${next}.`);
      return next;
    });
  }, [pushBanner]);

  const toggleLogView = useCallback(() => {
    setIsLogViewOpen((current) => {
      const next = !current;
      pushBanner("info", next ? "Log panel opened." : "Log panel closed.");
      return next;
    });
  }, [pushBanner]);

  const scrollLogsUp = useCallback(
    (step = LOG_SCROLL_STEP) => {
      setLogScrollOffset((current) =>
        Math.min(current + Math.max(1, step), Math.max(taskLogs.length - 1, 0)),
      );
    },
    [taskLogs.length],
  );

  const scrollLogsDown = useCallback((step = LOG_SCROLL_STEP) => {
    setLogScrollOffset((current) => Math.max(0, current - Math.max(1, step)));
  }, []);

  const scrollLogsToOldest = useCallback(() => {
    setLogScrollOffset(Math.max(taskLogs.length - 1, 0));
  }, [taskLogs.length]);

  const scrollLogsToLatest = useCallback(() => {
    setLogScrollOffset(0);
  }, []);

  const deleteSelectedTask = useCallback(async () => {
    const task = selectedTask;
    if (!task) {
      pushBanner("warn", "No task selected.");
      return;
    }

    setBusyMessage(`Deleting ${task.taskId}...`);
    try {
      const deleted = await services.orchestrator.deleteTask(task.taskId);
      if (!deleted) {
        pushBanner("warn", `Task ${task.taskId} was already removed.`);
        return;
      }

      setSessionMessagesByTaskID((current) => {
        if (!current[task.taskId]) {
          return current;
        }

        const next = { ...current };
        delete next[task.taskId];
        return next;
      });
      setPromptByTaskID((current) => {
        if (!current[task.taskId]) {
          return current;
        }

        const next = { ...current };
        delete next[task.taskId];
        return next;
      });
      setModelByTaskID((current) => {
        if (!(task.taskId in current)) {
          return current;
        }

        const next = { ...current };
        delete next[task.taskId];
        return next;
      });
      setTasks(services.orchestrator.listTasks());
      pushBanner(
        "success",
        `Deleted task ${task.taskId} and cleaned its worktree.`,
      );
    } catch (error) {
      pushBanner("error", toErrorMessage(error));
    } finally {
      setBusyMessage(undefined);
    }
  }, [selectedTask, pushBanner, services.orchestrator]);

  useEffect(() => {
    setLogScrollOffset(0);
  }, [selectedTask?.taskId]);

  useInput((input, key) => {
    const isInTextInputMode =
      newProjectPathInput !== undefined ||
      newTaskPromptInput !== undefined ||
      modelPickerOpen ||
      followUpPromptInput !== undefined;
    const wantsMoveUp =
      (key.upArrow || input === "k") && !key.ctrl && !key.meta;
    const wantsMoveDown =
      (key.downArrow || input === "j") && !key.ctrl && !key.meta;

    if (key.ctrl && input === "c") {
      exit();
      return;
    }

    if (!isInTextInputMode && input === "q") {
      exit();
      return;
    }

    if (loading) {
      return;
    }

    if (!isInTextInputMode && (input === "l" || input === "L")) {
      toggleLogView();
      return;
    }

    if (key.tab && !isLogViewOpen) {
      setRoute((current) => nextRoute(current));
      return;
    }

    if (isLogViewOpen) {
      if (input === "u") {
        scrollLogsUp(LOG_SCROLL_PAGE);
        return;
      }

      if (input === "d") {
        scrollLogsDown(LOG_SCROLL_PAGE);
        return;
      }

      if (input === "k") {
        scrollLogsUp(LOG_SCROLL_STEP);
        return;
      }

      if (input === "j") {
        scrollLogsDown(LOG_SCROLL_STEP);
        return;
      }

      if (input === "g") {
        scrollLogsToOldest();
        return;
      }

      if (input === "G") {
        scrollLogsToLatest();
        return;
      }

      if (input === "v" || input === "V") {
        toggleLogViewLevel();
        return;
      }

      return;
    }

    if (newProjectPathInput !== undefined) {
      if (key.escape) {
        setNewProjectPathInput(undefined);
        pushBanner("info", "Project creation cancelled.");
        return;
      }

      if (key.return) {
        const pathToCreate = newProjectPathInput;
        setNewProjectPathInput(undefined);
        void createProject(pathToCreate);
        return;
      }

      if (key.backspace || key.delete) {
        setNewProjectPathInput((current) =>
          current && current.length > 0 ? current.slice(0, -1) : "",
        );
        return;
      }

      if (
        input &&
        !key.ctrl &&
        !key.meta &&
        !key.upArrow &&
        !key.downArrow &&
        !key.leftArrow &&
        !key.rightArrow
      ) {
        setNewProjectPathInput((current) => `${current ?? ""}${input}`);
      }

      return;
    }

    if (newTaskPromptInput !== undefined) {
      if (key.escape) {
        setNewTaskPromptInput(undefined);
        pushBanner("info", "Task creation cancelled.");
        return;
      }

      if (key.return) {
        const promptToSubmit = newTaskPromptInput.trim();
        if (!promptToSubmit) {
          pushBanner("warn", "Task prompt is required.");
          return;
        }

        setNewTaskPromptInput(undefined);
        void runTask(promptToSubmit, taskModel);
        return;
      }

      if (key.backspace || key.delete) {
        setNewTaskPromptInput((current) =>
          current && current.length > 0 ? current.slice(0, -1) : "",
        );
        return;
      }

      if (
        input &&
        !key.ctrl &&
        !key.meta &&
        !key.upArrow &&
        !key.downArrow &&
        !key.leftArrow &&
        !key.rightArrow
      ) {
        setNewTaskPromptInput((current) => `${current ?? ""}${input}`);
      }

      return;
    }

    if (modelPickerOpen) {
      if (key.escape) {
        setModelPickerOpen(false);
        setModelFilterInput("");
        pushBanner("info", "Model selection cancelled.");
        return;
      }

      if (key.return) {
        const selectedOption = filteredModelOptions[selectedModelOptionIndex];
        if (!selectedOption) {
          pushBanner("warn", "No matching model option is selected.");
          return;
        }
        setTaskModel(selectedOption.model);
        setModelPickerOpen(false);
        setModelFilterInput("");
        pushBanner("success", `Task model set to ${selectedOption.label}.`);
        return;
      }

      if (key.upArrow && !key.ctrl && !key.meta) {
        setSelectedModelOptionIndex((current) => Math.max(0, current - 1));
        return;
      }

      if (key.downArrow && !key.ctrl && !key.meta) {
        setSelectedModelOptionIndex((current) =>
          Math.min(filteredModelOptions.length - 1, current + 1),
        );
        return;
      }

      if (key.backspace || key.delete) {
        setModelFilterInput((current) => current.slice(0, -1));
        setSelectedModelOptionIndex(0);
        return;
      }

      if (
        input &&
        !key.ctrl &&
        !key.meta &&
        !key.upArrow &&
        !key.downArrow &&
        !key.leftArrow &&
        !key.rightArrow &&
        input.length === 1
      ) {
        setModelFilterInput((current) => `${current}${input}`);
        setSelectedModelOptionIndex(0);
        return;
      }

      return;
    }

    if (followUpPromptInput !== undefined) {
      if (key.escape) {
        setFollowUpPromptInput(undefined);
        pushBanner("info", "Follow-up prompt cancelled.");
        return;
      }

      if (key.return) {
        const promptToSubmit = followUpPromptInput.trim();
        if (!promptToSubmit) {
          pushBanner("warn", "Follow-up prompt is required.");
          return;
        }

        setFollowUpPromptInput(undefined);
        void sendFollowUpPrompt(promptToSubmit);
        return;
      }

      if (key.backspace || key.delete) {
        setFollowUpPromptInput((current) =>
          current && current.length > 0 ? current.slice(0, -1) : "",
        );
        return;
      }

      if (
        input &&
        !key.ctrl &&
        !key.meta &&
        !key.upArrow &&
        !key.downArrow &&
        !key.leftArrow &&
        !key.rightArrow
      ) {
        setFollowUpPromptInput((current) => `${current ?? ""}${input}`);
      }

      return;
    }

    if (route === "project-selector") {
      if (wantsMoveUp) {
        setSelectedProjectIndex((current) => Math.max(0, current - 1));
        return;
      }

      if (wantsMoveDown) {
        setSelectedProjectIndex((current) =>
          Math.min(projects.length - 1, current + 1),
        );
        return;
      }

      if (key.return) {
        const project = projects[selectedProjectIndex];
        if (project) {
          void selectProject(project.id);
        }
        return;
      }

      if (input === "n") {
        startProjectCreationInput();
        return;
      }

      return;
    }

    if (wantsMoveUp) {
      setSelectedTaskIndex((current) => Math.max(0, current - 1));
      return;
    }

    if (wantsMoveDown) {
      setSelectedTaskIndex((current) =>
        Math.min(tasksForActiveProject.length - 1, current + 1),
      );
      return;
    }

    if (input === "n") {
      startTaskPromptInput();
      return;
    }

    if (input === "o") {
      void openTaskModelPicker();
      return;
    }

    if (input === "d") {
      void deleteSelectedTask();
      return;
    }

    if (input === "p") {
      startFollowUpPromptInput();
      return;
    }

    if (input === "m") {
      void mergeSelectedTask();
      return;
    }
  });

  const frameWidth = Math.max(stdout.columns ?? 40, 40);
  const frameHeight = Math.max(stdout.rows ?? 16, 16);
  const logVisibleRows = Math.max(
    isLogViewOpen ? frameHeight - 8 : Math.floor(frameHeight / 3),
    6,
  );

  return (
    <Box
      flexDirection="column"
      width={frameWidth}
      height={frameHeight}
      paddingX={1}
    >
      <Box marginBottom={1}>
        <Text color="cyanBright">iKanban</Text>
        <Text> - {ROUTE_DESCRIPTORS[route].title}</Text>
        <Text color={services.runtime.isRunning() ? "green" : "red"}>
          {" "}
          | runtime {services.runtime.isRunning() ? "up" : "down"}
        </Text>
      </Box>

      {statusBanner ? (
        <Box marginBottom={1}>
          <Text color={toInkColor(statusBanner.tone)}>
            [{statusBanner.tone.toUpperCase()}] {statusBanner.message} (
            {formatTime(statusBanner.at)})
          </Text>
        </Box>
      ) : null}

      {errorMessage ? (
        <Box marginBottom={1}>
          <Text color="red">Error: {errorMessage}</Text>
        </Box>
      ) : null}

      <Box flexDirection="column" flexGrow={1}>
        {loading ? (
          <Text color="yellow">Loading runtime and project state...</Text>
        ) : isLogViewOpen ? (
          <Box flexDirection="column" flexGrow={1}>
            <LogView
              entries={taskLogs}
              level={logViewLevel}
              scrollOffset={logScrollOffset}
              visibleRows={logVisibleRows}
            />
          </Box>
        ) : route === "project-selector" ? (
          <Box flexDirection="column" flexGrow={1}>
            <Text color="magentaBright">Projects</Text>
            <Box marginTop={1} flexDirection="column">
              <ProjectSelectorView
                projects={projects}
                selectedProjectIndex={selectedProjectIndex}
              />
            </Box>
          </Box>
        ) : (
          <Box flexDirection="column" flexGrow={1}>
            <Box flexDirection="column">
              <Text color="magentaBright">
                Tasks ({activeProject?.name ?? "none"})
              </Text>
              <Box marginTop={1} flexDirection="column">
                <TaskBoardView
                  tasks={tasksForActiveProject}
                  selectedTaskIndex={selectedTaskIndex}
                  pendingTaskModelLabel={formatModel(taskModel)}
                />
              </Box>
            </Box>

            <Box marginTop={1} flexDirection="column" flexGrow={1}>
              <Text color="magentaBright">Details</Text>
              <Box marginTop={1} flexDirection="column">
                {selectedTask ? (
                  <>
                    <Text>Task: {selectedTask.taskId}</Text>
                    <Text>State: {selectedTask.state}</Text>
                    <Text>Project: {selectedTask.projectId}</Text>
                    <Text>
                      Model: {formatModel(modelByTaskID[selectedTask.taskId])}
                    </Text>
                    <Text>Session: {selectedTask.sessionID ?? "-"}</Text>
                    <Text>
                      Worktree: {selectedTask.worktreeDirectory ?? "-"}
                    </Text>
                    <Text>Error: {selectedTask.error ?? "-"}</Text>
                  </>
                ) : (
                  <Text color="yellow">Select a task to inspect details.</Text>
                )}
              </Box>

              <Box marginTop={1} flexDirection="column">
                <Text color="cyan">Conversation</Text>
                {taskMessages.length > 0 ? (
                  taskMessages.slice(-6).map((message) => (
                    <Text
                      key={message.messageID}
                      color={message.role === "assistant" ? "green" : undefined}
                    >
                      [{message.role}]{" "}
                      {truncate(message.preview || "(no text preview)", 120)}
                    </Text>
                  ))
                ) : (
                  <Text color="yellow">No conversation messages yet.</Text>
                )}
              </Box>
            </Box>
          </Box>
        )}
      </Box>

      {newProjectPathInput !== undefined ? (
        <Box marginTop={1}>
          <Text color="cyan">
            New project path: {newProjectPathInput || " "}
          </Text>
        </Box>
      ) : null}

      {newTaskPromptInput !== undefined ? (
        <Box marginTop={1}>
          <Text color="cyan">New task prompt: {newTaskPromptInput || " "}</Text>
        </Box>
      ) : null}

      {modelPickerOpen ? (
        <Box marginTop={1} flexDirection="column">
          <Text color="cyan">Select task model (Enter save, Esc cancel)</Text>
          <Text color="gray">Filter: {modelFilterInput || "(none)"}</Text>
          {filteredModelOptions.length > 0 ? (
            visibleModelOptions(
              filteredModelOptions,
              selectedModelOptionIndex,
              8,
            ).map((option) => (
              <Text
                key={option.label}
                color={option.isSelected ? "green" : undefined}
              >
                {option.isSelected ? ">" : " "} {option.label}
              </Text>
            ))
          ) : (
            <Text color="yellow">(no matching models)</Text>
          )}
        </Box>
      ) : null}

      {followUpPromptInput !== undefined ? (
        <Box marginTop={1}>
          <Text color="cyan">
            Follow-up prompt: {followUpPromptInput || " "}
          </Text>
        </Box>
      ) : null}

      <Box marginTop={1}>
        <Text color="gray">
          {keyboardHints(route, {
            isCreatingProject: newProjectPathInput !== undefined,
            isCreatingTask: newTaskPromptInput !== undefined,
            isEditingTaskModel: modelPickerOpen,
            isFollowUpPrompt: followUpPromptInput !== undefined,
            logViewLevel,
            isLogViewOpen,
          })}
        </Text>
      </Box>

      {busyMessage ? (
        <Box marginTop={1}>
          <Text color="yellow">{busyMessage}</Text>
        </Box>
      ) : null}
    </Box>
  );
}

function keyboardHints(
  route: AppRoute,
  options: {
    isCreatingProject: boolean;
    isCreatingTask: boolean;
    isEditingTaskModel: boolean;
    isFollowUpPrompt: boolean;
    logViewLevel: LogViewLevel;
    isLogViewOpen: boolean;
  },
): string {
  if (options.isLogViewOpen) {
    return `Keys: j/k line | u/d page | g/G oldest/latest | v log ${options.logViewLevel} | l close logs | q quit`;
  }

  if (route === "project-selector") {
    return options.isCreatingProject
      ? "Keys: Type path | Enter create | Esc cancel"
      : "Keys: Up/Down or k/j move | Enter select | n new project | l open logs | Tab switch | q quit";
  }

  if (options.isFollowUpPrompt) {
    return "Keys: Type prompt | Enter send | Esc cancel";
  }

  if (options.isEditingTaskModel) {
    return "Keys: Type to filter | Up/Down move | Backspace delete | Enter save | Esc cancel";
  }

  return options.isCreatingTask
    ? "Keys: Type prompt | Enter run | Esc cancel"
    : "Keys: Up/Down or k/j move | n new task | o task model | p follow-up prompt | m merge | d delete | l logs | Tab switch | q quit";
}

async function ensureDefaultProject(
  registry: ProjectRegistry,
  defaultProjectDirectory?: string,
): Promise<void> {
  const existingProjects = await registry.listProjects();
  if (existingProjects.length > 0) {
    return;
  }

  const rootDirectory = resolve(defaultProjectDirectory ?? process.cwd());
  const projectName = basename(rootDirectory) || "project";
  const projectID = toSlug(projectName) || `project-${Date.now()}`;

  await registry.addProject({
    id: projectID,
    name: projectName,
    rootDirectory,
  });
}

function relayOrchestratorEvent(
  event: TaskOrchestratorEvent,
  bus: RuntimeEventBus,
  resolveProjectID: (taskID: string) => string,
): void {
  switch (event.type) {
    case "task.enqueued": {
      bus.emit("task.created", {
        taskId: event.task.taskId,
        projectId: event.task.projectId,
        state: event.task.state,
        createdAt: event.task.createdAt,
      });
      return;
    }
    case "task.state.changed": {
      if (event.to === "completed") {
        bus.emit("task.completed", {
          taskId: event.task.taskId,
          projectId: event.task.projectId,
          completedAt: event.task.updatedAt,
        });
      }
      return;
    }
    case "task.worktree.created": {
      bus.emit("worktree.created", {
        taskId: event.taskId,
        projectId: resolveProjectID(event.taskId),
        directory: event.worktree.directory,
        branch: event.worktree.branch,
        name: event.worktree.name,
        createdAt: event.worktree.createdAt,
      });
      return;
    }
    case "task.session.created": {
      bus.emit("session.created", {
        taskId: event.taskId,
        projectId: event.session.projectId,
        sessionID: event.session.sessionID,
        directory: event.session.directory,
        createdAt: event.session.createdAt,
        title: event.session.title,
      });
      return;
    }
    case "task.prompt.submitted": {
      const session = event.prompt.sessionID;
      bus.emit("session.prompt.submitted", {
        taskId: event.taskId,
        projectId: resolveProjectID(event.taskId),
        sessionID: session,
        prompt: event.prompt.prompt,
        submittedAt: event.prompt.submittedAt,
      });
      return;
    }
    case "task.session.message.received": {
      bus.emit("session.message.received", {
        taskId: event.taskId,
        projectId: resolveProjectID(event.taskId),
        sessionID: event.sessionID,
        messageID: event.message.id,
        createdAt: event.message.createdAt,
        preview: event.message.preview,
        role: event.message.role,
      });
      return;
    }
    case "task.cleanup.completed": {
      bus.emit("worktree.cleanup", {
        taskId: event.taskId,
        projectId: event.task.projectId,
        policy: event.cleanup.policy,
        worktreeDirectory: event.cleanup.worktreeDirectory,
        removed: event.cleanup.removed,
        updatedAt: Date.now(),
      });

      if (event.cleanup.removed && event.cleanup.worktreeDirectory) {
        bus.emit("worktree.removed", {
          taskId: event.taskId,
          projectId: event.task.projectId,
          directory: event.cleanup.worktreeDirectory,
          removedAt: Date.now(),
        });
      }
      return;
    }
    case "task.failed": {
      bus.emit("task.failed", {
        taskId: event.taskId,
        projectId: event.task.projectId,
        failedAt: event.task.updatedAt,
        error: event.error,
      });
      return;
    }
    case "task.review": {
      bus.emit("task.state.updated", {
        taskId: event.taskId,
        projectId: event.task.projectId,
        previousState: "running",
        nextState: "review",
        updatedAt: event.task.updatedAt,
      });
      return;
    }
    case "task.merged": {
      return;
    }
  }
}

function createTaskID(seed: string, prompt?: string): string {
  const hash = simpleHash(seed + Date.now().toString()).slice(0, 7);
  const promptSlice = prompt ? toSlug(prompt).slice(0, 16) : "";
  return promptSlice ? `${promptSlice}-${hash}` : `task-${hash}`;
}

function simpleHash(input: string): string {
  let h = 0;
  for (let i = 0; i < input.length; i++) {
    h = ((h << 5) - h + input.charCodeAt(i)) | 0;
  }
  return Math.abs(h).toString(36);
}

function buildDefaultPrompt(projectName: string, taskID: string): string {
  return `Work on task ${taskID} in project ${projectName}. Analyze the repository and implement the next meaningful change with tests.`;
}

function buildRetryPrompt(taskID: string): string {
  return `Retry task ${taskID}. Address the previous failure and continue with the same objective.`;
}

function formatModel(model: PromptModel | undefined): string {
  if (!model) {
    return "default";
  }

  return `${model.providerID}/${model.modelID}`;
}

function isSameModel(
  left: PromptModel | undefined,
  right: PromptModel | undefined,
): boolean {
  if (!left && !right) {
    return true;
  }

  if (!left || !right) {
    return false;
  }

  return left.providerID === right.providerID && left.modelID === right.modelID;
}

async function loadModelOptions(
  services: AppServices,
  directory: string,
): Promise<ModelOption[]> {
  const client = await services.runtime.getClient(directory);
  const response = await client.config.providers({ directory });

  if (response.error) {
    throw new Error(
      `Failed to load models: ${formatUnknownError(response.error)}`,
    );
  }

  const data = response.data as
    | {
      providers?: Array<{
        id?: string;
        models?: Record<string, unknown>;
      }>;
    }
    | undefined;

  const options: ModelOption[] = [{ label: "default", model: undefined }];
  const seen = new Set<string>();

  for (const provider of data?.providers ?? []) {
    const providerID = provider.id?.trim();
    if (!providerID || !provider.models) {
      continue;
    }

    for (const modelID of Object.keys(provider.models)) {
      const key = `${providerID}/${modelID}`;
      if (seen.has(key)) {
        continue;
      }

      seen.add(key);
      options.push({
        label: key,
        model: {
          providerID,
          modelID,
        },
      });
    }
  }

  if (options.length === 1) {
    return options;
  }

  const explicit = options
    .slice(1)
    .sort((left, right) => left.label.localeCompare(right.label));
  const [defaultOption] = options;
  return defaultOption ? [defaultOption, ...explicit] : explicit;
}

function visibleModelOptions(
  options: ModelOption[],
  selectedIndex: number,
  maxRows: number,
): Array<ModelOption & { isSelected: boolean }> {
  if (options.length === 0) {
    return [];
  }

  const clampedRows = Math.max(1, maxRows);
  const safeSelectedIndex = Math.max(
    0,
    Math.min(selectedIndex, options.length - 1),
  );
  if (options.length <= clampedRows) {
    return options.map((option, index) => ({
      ...option,
      isSelected: index === safeSelectedIndex,
    }));
  }

  const half = Math.floor(clampedRows / 2);
  const start = Math.max(
    0,
    Math.min(safeSelectedIndex - half, options.length - clampedRows),
  );
  const end = start + clampedRows;
  return options.slice(start, end).map((option, index) => ({
    ...option,
    isSelected: start + index === safeSelectedIndex,
  }));
}

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Unknown SDK error";
}

function filterModelOptions(
  options: ModelOption[],
  query: string,
): ModelOption[] {
  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery) {
    return options;
  }

  return options.filter((option) =>
    fuzzyMatches(option.label.toLowerCase(), normalizedQuery),
  );
}

function fuzzyMatches(text: string, query: string): boolean {
  if (!query) {
    return true;
  }

  let textIndex = 0;
  let queryIndex = 0;

  while (textIndex < text.length && queryIndex < query.length) {
    if (text[textIndex] === query[queryIndex]) {
      queryIndex += 1;
    }
    textIndex += 1;
  }

  return queryIndex === query.length;
}

function toSlug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+/, "")
    .replace(/-+$/, "")
    .slice(0, 36);
}

function buildUniqueProjectID(
  projectName: string,
  existingProjectIDs: string[],
): string {
  const base = toSlug(projectName) || "project";
  const existing = new Set(existingProjectIDs);

  if (!existing.has(base)) {
    return base;
  }

  for (let index = 2; index < 10_000; index += 1) {
    const candidate = `${base}-${index}`.slice(0, 36);
    if (!existing.has(candidate)) {
      return candidate;
    }
  }

  return `${base}-${Date.now()}`.slice(0, 36);
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Unknown error";
}

function toInkColor(tone: BannerTone): "blue" | "green" | "yellow" | "red" {
  switch (tone) {
    case "info":
      return "blue";
    case "success":
      return "green";
    case "warn":
      return "yellow";
    case "error":
      return "red";
  }
}

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString();
}

function truncate(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength - 3)}...`;
}
