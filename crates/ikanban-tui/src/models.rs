// Re-export core entities
pub use ikanban_core::entities::execution_process::Model as ExecutionProcess;
pub use ikanban_core::entities::execution_process_logs::Model as ExecutionProcessLog;
pub use ikanban_core::entities::project::{
    CreateProject, Model as Project, ProjectWithStatus, UpdateProject,
};
pub use ikanban_core::entities::response::{
    ApiResponse, SubscribeTarget, WsEvent, WsMessage, WsRequest, WsResponse, WsResponseData,
};
pub use ikanban_core::entities::session::Model as Session;
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
            TaskStatus::InReview => "In Review",
            TaskStatus::Done => "Done",
            TaskStatus::Cancelled => "Cancelled",
        }
    }

    fn next(&self) -> TaskStatus {
        match self {
            TaskStatus::Todo => TaskStatus::InProgress,
            TaskStatus::InProgress => TaskStatus::InReview,
            TaskStatus::InReview => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Todo,
            TaskStatus::Cancelled => TaskStatus::Todo,
        }
    }
}
