use crate::entities::{direct_merge, execution_process, pr_merge, project, session, task};
use serde::{Deserialize, Serialize};

/// Standard API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.to_string()),
        }
    }
}

/// WebSocket event message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsEvent {
    // Project events
    ProjectCreated(project::Model),
    ProjectUpdated(project::Model),
    ProjectDeleted {
        id: uuid::Uuid,
    },

    // Task events
    TaskCreated(task::Model),
    TaskUpdated(task::Model),
    TaskDeleted {
        id: uuid::Uuid,
    },

    // Session events
    SessionCreated(session::Model),
    SessionUpdated(session::Model),

    // Execution events
    ExecutionCreated(execution_process::Model),
    ExecutionUpdated(execution_process::Model),

    // Merge events
    DirectMergeCreated(direct_merge::Model),
    PrMergeCreated(pr_merge::Model),
    PrMergeUpdated(pr_merge::Model),

    // Log events
    Log {
        execution_id: uuid::Uuid,
        content: String,
    },

    // Connection events
    Connected,
    Ping,
    Pong,
}
