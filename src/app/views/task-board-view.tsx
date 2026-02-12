import { Text } from "ink";

import type { TaskRuntime, TaskState } from "../../domain/task";

type TaskBoardViewProps = {
  tasks: TaskRuntime[];
  selectedTaskIndex: number;
};

export function TaskBoardView({ tasks, selectedTaskIndex }: TaskBoardViewProps) {
  if (tasks.length === 0) {
    return <Text color="yellow">No tasks for active project.</Text>;
  }

  return (
    <>
      <Text color="gray">Press d to delete selected task and clean worktree.</Text>
      {tasks.map((task, index) => {
        return (
          <Text key={task.taskId} color={index === selectedTaskIndex ? "green" : stateColor(task.state)}>
            {index === selectedTaskIndex ? ">" : " "} {task.taskId} [{task.state}]
          </Text>
        );
      })}
    </>
  );
}

function stateColor(state: TaskState): "yellow" | "cyan" | "green" | "red" | undefined {
  switch (state) {
    case "queued":
      return "yellow";
    case "creating_worktree":
      return "yellow";
    case "running":
      return "cyan";
    case "completed":
      return "green";
    case "failed":
      return "red";
    case "cleaning":
      return "yellow";
  }
}
