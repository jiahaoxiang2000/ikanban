use crate::entities::{
    direct_merge, execution_process, execution_process_logs, pr_merge, project, session, task,
};
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

/// WebSocket message types (requests and events)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    // Connection
    Connected,
    Ping,
    Pong,

    // Requests (client -> server)
    Request {
        id: String,
        #[serde(flatten)]
        request: WsRequest,
    },

    // Responses (server -> client)
    Response {
        id: String,
        #[serde(flatten)]
        response: WsResponse,
    },

    // Events (server -> client, broadcast)
    Event(WsEvent),
}

/// Client requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "data")]
pub enum WsRequest {
    // Projects
    ListProjects,
    GetProject {
        id: uuid::Uuid,
    },
    CreateProject(project::CreateProject),
    UpdateProject {
        id: uuid::Uuid,
        data: project::UpdateProject,
    },
    DeleteProject {
        id: uuid::Uuid,
    },

    // Tasks
    ListTasks {
        project_id: uuid::Uuid,
    },
    GetTask {
        id: uuid::Uuid,
    },
    CreateTask(task::CreateTask),
    UpdateTask {
        id: uuid::Uuid,
        data: task::UpdateTask,
    },
    DeleteTask {
        id: uuid::Uuid,
    },

    // Sessions
    ListSessions {
        task_id: uuid::Uuid,
    },
    GetSession {
        task_id: uuid::Uuid,
        id: uuid::Uuid,
    },
    CreateSession(session::CreateSession),

    // Executions
    ListExecutions {
        session_id: uuid::Uuid,
    },
    GetExecution {
        id: uuid::Uuid,
    },
    CreateExecution(execution_process::CreateExecutionProcess),
    StopExecution {
        id: uuid::Uuid,
    },

    // Execution Logs
    GetExecutionLogs {
        execution_id: uuid::Uuid,
        limit: Option<u64>,
    },
    CreateExecutionLog(execution_process_logs::CreateExecutionProcessLog),

    // Subscribe to specific streams
    Subscribe(SubscribeTarget),
    Unsubscribe(SubscribeTarget),
}

/// Subscription targets
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "target", content = "filter")]
pub enum SubscribeTarget {
    Projects,
    Tasks { project_id: uuid::Uuid },
    ExecutionLogs { execution_id: uuid::Uuid },
}

/// Server responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum WsResponse {
    Success(WsResponseData),
    Error { message: String },
}

/// Response data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WsResponseData {
    Projects(Vec<project::ProjectWithStatus>),
    Project(project::Model),
    Tasks(Vec<task::TaskWithSessionStatus>),
    Task(task::Model),
    Sessions(Vec<session::Model>),
    Session(session::Model),
    Executions(Vec<execution_process::Model>),
    Execution(execution_process::Model),
    ExecutionLogs(Vec<execution_process_logs::Model>),
    Empty,
}

/// Broadcast events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload")]
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
}
