// Re-export core entities
pub use ikanban_core::entities::project::{
    CreateProject, Model as Project, ProjectWithStatus, UpdateProject,
};
pub use ikanban_core::entities::response::{ApiResponse, WsEvent};
pub use ikanban_core::entities::task::{CreateTask, Model as Task, TaskStatus, UpdateTask};

/// Extension trait for TaskStatus with TUI-specific display helpers
pub trait TaskStatusExt {
    fn as_str(&self) -> &'static str;
    fn next(&self) -> TaskStatus;
}

impl TaskStatusExt for TaskStatus {
    fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "Todo",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::Done => "Done",
        }
    }

    fn next(&self) -> TaskStatus {
        match self {
            TaskStatus::Todo => TaskStatus::InProgress,
            TaskStatus::InProgress => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Todo,
        }
    }
}
