use serde::{Deserialize, Serialize};
use strum::Display;

use crate::models::TaskStatus;

/// Application actions that can be triggered by events
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    // Tick and Render
    Tick,
    Render,
    Resize(u16, u16),

    // Terminal actions
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,

    // Navigation
    NextProject,
    PreviousProject,
    NextTask,
    PreviousTask,
    NextColumn,
    PreviousColumn,

    // View management
    EnterProjectsView,
    EnterProjectDetailView,
    EnterTasksView,
    EnterTaskDetailView,
    EnterExecutionLogsView,
    LeaveExecutionLogsView,
    ToggleHelpModal,
    CloseHelpModal,

    // Input mode
    StartInput(InputField),
    StartInputForNew(InputField),
    StartInputForEdit(InputField),
    CancelInput,
    SubmitInput,
    InsertChar(char),
    InsertNewline,
    DeleteBackward,
    DeleteForward,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorUp,
    MoveCursorDown,
    MoveCursorHome,
    MoveCursorEnd,

    // API operations
    LoadProjects,
    LoadTasks,
    CreateProject(String),
    UpdateProject,
    DeleteSelectedProject,
    CreateTask(String),
    UpdateTask,
    DeleteSelectedTask,
    MoveTaskToNextStatus,
    LoadExecutions,
    RefreshExecutionLogs,
    StopSelectedExecution,

    // WebSocket
    ConnectProjectsWs,
    ConnectTasksWs,
    DisconnectTasksWs,
    ConnectExecutionLogsWs,
    DisconnectExecutionLogsWs,
    ProcessWsEvents,
}

/// Which field is being edited
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputField {
    None,
    ProjectName,
    ProjectDescription,
    ProjectRepoPath,
    TaskTitle,
    TaskDescription,
}

impl Default for InputField {
    fn default() -> Self {
        InputField::None
    }
}
