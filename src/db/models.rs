use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for Project {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Project {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            path: PathBuf::from(row.try_get::<String, _>("path")?),
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::InReview => write!(f, "in_review"),
            TaskStatus::Done => write!(f, "done"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(TaskStatus::Todo),
            "in_progress" => Ok(TaskStatus::InProgress),
            "in_review" => Ok(TaskStatus::InReview),
            "done" => Ok(TaskStatus::Done),
            _ => Err(format!("Invalid task status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for Task {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        let status_str: String = row.try_get("status")?;
        let status = status_str.parse().map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })?;

        Ok(Task {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            title: row.try_get("title")?,
            description: row.try_get("description")?,
            status,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Running => write!(f, "running"),
            SessionStatus::Completed => write!(f, "completed"),
            SessionStatus::Failed => write!(f, "failed"),
            SessionStatus::Killed => write!(f, "killed"),
        }
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(SessionStatus::Running),
            "completed" => Ok(SessionStatus::Completed),
            "failed" => Ok(SessionStatus::Failed),
            "killed" => Ok(SessionStatus::Killed),
            _ => Err(format!("Invalid session status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub task_id: String,

    // Worktree lifecycle
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,

    // Execution info
    pub executor_type: String,
    pub status: SessionStatus,
    pub exit_code: Option<i32>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for Session {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        let status_str: String = row.try_get("status")?;
        let status = status_str.parse().map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })?;

        let worktree_path: Option<String> = row.try_get("worktree_path")?;

        Ok(Session {
            id: row.try_get("id")?,
            task_id: row.try_get("task_id")?,
            worktree_path: worktree_path.map(PathBuf::from),
            branch_name: row.try_get("branch_name")?,
            executor_type: row.try_get("executor_type")?,
            status,
            exit_code: row.try_get("exit_code")?,
            created_at: row.try_get("created_at")?,
            started_at: row.try_get("started_at")?,
            finished_at: row.try_get("finished_at")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum LogType {
    Stdout,
    Stderr,
    Event,
}

impl std::fmt::Display for LogType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogType::Stdout => write!(f, "stdout"),
            LogType::Stderr => write!(f, "stderr"),
            LogType::Event => write!(f, "event"),
        }
    }
}

impl std::str::FromStr for LogType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stdout" => Ok(LogType::Stdout),
            "stderr" => Ok(LogType::Stderr),
            "event" => Ok(LogType::Event),
            _ => Err(format!("Invalid log type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub log_type: LogType,
    pub content: String,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for LogEntry {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        let log_type_str: String = row.try_get("log_type")?;
        let log_type = log_type_str.parse().map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })?;

        Ok(LogEntry {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            timestamp: row.try_get("timestamp")?,
            log_type,
            content: row.try_get("content")?,
        })
    }
}
