use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, Pool, Sqlite};
use std::env;
use std::path::PathBuf;

pub type DbPool = Pool<Sqlite>;

pub async fn init_pool() -> Result<DbPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ikanban");

        std::fs::create_dir_all(&data_dir).ok();

        format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("ikanban.db").display()
        )
    });

    tracing::info!("Connecting to database: {}", database_url);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
}

pub async fn run_migrations(pool: &DbPool) -> Result<(), sqlx::migrate::MigrateError> {
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(pool).await
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub repo_path: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub id: String,
    pub name: String,
    pub repo_path: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub repo_path: Option<String>,
    pub description: Option<String>,
}

impl Project {
    pub async fn create(pool: &DbPool, input: CreateProject) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO projects (id, name, repo_path, description)
            VALUES (?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.repo_path)
        .bind(&input.description)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &DbPool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn list(pool: &DbPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM projects ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
    }

    pub async fn update(pool: &DbPool, id: &str, input: UpdateProject) -> Result<Self, sqlx::Error> {
        let current = Self::get_by_id(pool, id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        sqlx::query_as::<_, Self>(
            r#"
            UPDATE projects
            SET name = ?, repo_path = ?, description = ?, updated_at = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(input.name.unwrap_or(current.name))
        .bind(input.repo_path.unwrap_or(current.repo_path))
        .bind(input.description.or(current.description))
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &DbPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::InReview => "in_review",
            Self::Done => "done",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "in_progress" => Some(Self::InProgress),
            "in_review" => Some(Self::InReview),
            "done" => Some(Self::Done),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTask {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
}

impl Task {
    pub async fn create(pool: &DbPool, input: CreateTask) -> Result<Self, sqlx::Error> {
        let status = input.status.unwrap_or(TaskStatus::Todo).as_str();
        let priority = input.priority.unwrap_or(TaskPriority::Medium).as_str();

        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO tasks (id, project_id, title, description, status, priority)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&input.id)
        .bind(&input.project_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(status)
        .bind(priority)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &DbPool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn list_by_project(pool: &DbPool, project_id: &str) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM tasks WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
    }

    pub async fn list_by_project_and_status(
        pool: &DbPool,
        project_id: &str,
        status: TaskStatus,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM tasks WHERE project_id = ? AND status = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .bind(status.as_str())
        .fetch_all(pool)
        .await
    }

    pub async fn update(pool: &DbPool, id: &str, input: UpdateTask) -> Result<Self, sqlx::Error> {
        let current = Self::get_by_id(pool, id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        let status = input
            .status
            .map(|s| s.as_str().to_string())
            .unwrap_or(current.status);
        let priority = input
            .priority
            .map(|p| p.as_str().to_string())
            .unwrap_or(current.priority);

        sqlx::query_as::<_, Self>(
            r#"
            UPDATE tasks
            SET title = ?, description = ?, status = ?, priority = ?, updated_at = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(input.title.unwrap_or(current.title))
        .bind(input.description.or(current.description))
        .bind(status)
        .bind(priority)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &DbPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn can_transition_to(&self, target: Self) -> bool {
        use SessionStatus::*;
        matches!(
            (self, target),
            (Pending, Running)
                | (Pending, Cancelled)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Cancelled)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub task_id: String,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub status: String,
    pub created_at: String,
    pub ended_at: Option<String>,
}

impl Session {
    pub fn status_enum(&self) -> SessionStatus {
        SessionStatus::from_str(&self.status).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSession {
    pub id: String,
    pub task_id: String,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub status: Option<SessionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSession {
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub status: Option<SessionStatus>,
    pub ended_at: Option<String>,
}

impl Session {
    pub async fn create(pool: &DbPool, input: CreateSession) -> Result<Self, sqlx::Error> {
        let status = input.status.unwrap_or(SessionStatus::Pending).as_str();

        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO sessions (id, task_id, worktree_path, branch_name, status)
            VALUES (?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&input.id)
        .bind(&input.task_id)
        .bind(&input.worktree_path)
        .bind(&input.branch_name)
        .bind(status)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &DbPool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn list_by_task(pool: &DbPool, task_id: &str) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM sessions WHERE task_id = ? ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(pool)
        .await
    }

    pub async fn list_by_status(
        pool: &DbPool,
        status: SessionStatus,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM sessions WHERE status = ? ORDER BY created_at DESC")
            .bind(status.as_str())
            .fetch_all(pool)
            .await
    }

    pub async fn update(pool: &DbPool, id: &str, input: UpdateSession) -> Result<Self, sqlx::Error> {
        let current = Self::get_by_id(pool, id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        let status = input
            .status
            .map(|s| s.as_str().to_string())
            .unwrap_or(current.status);

        sqlx::query_as::<_, Self>(
            r#"
            UPDATE sessions
            SET worktree_path = ?, branch_name = ?, status = ?, ended_at = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(input.worktree_path.or(current.worktree_path))
        .bind(input.branch_name.or(current.branch_name))
        .bind(status)
        .bind(input.ended_at.or(current.ended_at))
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &DbPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl ExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionProcess {
    pub id: String,
    pub session_id: String,
    pub run_reason: String,
    pub pid: Option<i64>,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

impl ExecutionProcess {
    pub fn status_enum(&self) -> ExecutionStatus {
        ExecutionStatus::from_str(&self.status).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateExecutionProcess {
    pub id: String,
    pub session_id: String,
    pub run_reason: Option<String>,
    pub pid: Option<i64>,
    pub status: Option<ExecutionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateExecutionProcess {
    pub pid: Option<i64>,
    pub status: Option<ExecutionStatus>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

impl ExecutionProcess {
    pub async fn create(pool: &DbPool, input: CreateExecutionProcess) -> Result<Self, sqlx::Error> {
        let status = input.status.unwrap_or(ExecutionStatus::Pending).as_str();
        let run_reason = input.run_reason.unwrap_or_else(|| "coding_agent".to_string());

        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO execution_processes (id, session_id, run_reason, pid, status)
            VALUES (?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&input.id)
        .bind(&input.session_id)
        .bind(&run_reason)
        .bind(input.pid)
        .bind(status)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &DbPool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM execution_processes WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn list_by_session(pool: &DbPool, session_id: &str) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM execution_processes WHERE session_id = ? ORDER BY started_at DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await
    }

    pub async fn list_by_status(
        pool: &DbPool,
        status: ExecutionStatus,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM execution_processes WHERE status = ? ORDER BY started_at DESC",
        )
        .bind(status.as_str())
        .fetch_all(pool)
        .await
    }

    pub async fn update(
        pool: &DbPool,
        id: &str,
        input: UpdateExecutionProcess,
    ) -> Result<Self, sqlx::Error> {
        let current = Self::get_by_id(pool, id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        let status = input
            .status
            .map(|s| s.as_str().to_string())
            .unwrap_or(current.status);

        sqlx::query_as::<_, Self>(
            r#"
            UPDATE execution_processes
            SET pid = ?, status = ?, started_at = ?, ended_at = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(input.pid.or(current.pid))
        .bind(status)
        .bind(input.started_at.or(current.started_at))
        .bind(input.ended_at.or(current.ended_at))
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &DbPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM execution_processes WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodingAgentTurn {
    pub id: String,
    pub execution_id: String,
    pub turn_number: i64,
    pub input: Option<String>,
    pub output: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCodingAgentTurn {
    pub id: String,
    pub execution_id: String,
    pub turn_number: i64,
    pub input: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCodingAgentTurn {
    pub input: Option<String>,
    pub output: Option<String>,
}

impl CodingAgentTurn {
    pub async fn create(pool: &DbPool, input: CreateCodingAgentTurn) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO coding_agent_turns (id, execution_id, turn_number, input, output)
            VALUES (?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&input.id)
        .bind(&input.execution_id)
        .bind(input.turn_number)
        .bind(&input.input)
        .bind(&input.output)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &DbPool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM coding_agent_turns WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn list_by_execution(
        pool: &DbPool,
        execution_id: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM coding_agent_turns WHERE execution_id = ? ORDER BY turn_number ASC",
        )
        .bind(execution_id)
        .fetch_all(pool)
        .await
    }

    pub async fn get_latest_by_execution(
        pool: &DbPool,
        execution_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM coding_agent_turns WHERE execution_id = ? ORDER BY turn_number DESC LIMIT 1",
        )
        .bind(execution_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn update(
        pool: &DbPool,
        id: &str,
        input: UpdateCodingAgentTurn,
    ) -> Result<Self, sqlx::Error> {
        let current = Self::get_by_id(pool, id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        sqlx::query_as::<_, Self>(
            r#"
            UPDATE coding_agent_turns
            SET input = ?, output = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(input.input.or(current.input))
        .bind(input.output.or(current.output))
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &DbPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM coding_agent_turns WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
