use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

use crate::db::connection::create_pool;
use crate::db::models::{LogEntry, LogType, Project, Session, Task, TaskStatus};
use crate::executor::{Executor, LogMsg, OpenCodeExecutor};
use crate::session::SessionManager;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct AppState {
    pool: SqlitePool,
    session_manager: Arc<SessionManager>,
    executor: Arc<dyn Executor>,
}

impl AppState {
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        let pool = create_pool(&db_path)
            .await
            .context("Failed to create database pool")?;

        let session_manager = Arc::new(SessionManager::new(pool.clone()));
        let executor = Arc::new(OpenCodeExecutor::new());

        Ok(Self {
            pool,
            session_manager,
            executor,
        })
    }

    pub async fn create_project(&self, name: String, path: PathBuf) -> Result<Project> {
        let project_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let project = Project {
            id: project_id.clone(),
            name: name.clone(),
            path: path.clone(),
            created_at: now,
        };

        sqlx::query(
            r#"
            INSERT INTO projects (id, name, path, created_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(project.path.to_string_lossy().to_string())
        .bind(&project.created_at)
        .execute(&self.pool)
        .await
        .context("Failed to insert project")?;

        Ok(project)
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Project> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT * FROM projects WHERE id = ?
            "#,
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .context("Project not found")?;

        Ok(project)
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT * FROM projects ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch projects")?;

        Ok(projects)
    }

    pub async fn update_project(
        &self,
        project_id: &str,
        name: String,
        path: PathBuf,
    ) -> Result<Project> {
        sqlx::query(
            r#"
            UPDATE projects SET name = ?, path = ? WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(path.to_string_lossy().to_string())
        .bind(project_id)
        .execute(&self.pool)
        .await
        .context("Failed to update project")?;

        self.get_project(project_id).await
    }

    pub async fn delete_project(&self, project_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM projects WHERE id = ?
            "#,
        )
        .bind(project_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete project")?;

        Ok(())
    }

    pub async fn create_task(
        &self,
        project_id: String,
        title: String,
        description: Option<String>,
    ) -> Result<Task> {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let task = Task {
            id: task_id.clone(),
            project_id: project_id.clone(),
            title: title.clone(),
            description: description.clone(),
            status: TaskStatus::Todo,
            created_at: now,
        };

        sqlx::query(
            r#"
            INSERT INTO tasks (id, project_id, title, description, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&task.id)
        .bind(&task.project_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(task.status.to_string())
        .bind(&task.created_at)
        .execute(&self.pool)
        .await
        .context("Failed to insert task")?;

        Ok(task)
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Task> {
        let task = sqlx::query_as::<_, Task>(
            r#"
            SELECT * FROM tasks WHERE id = ?
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .context("Task not found")?;

        Ok(task)
    }

    pub async fn list_tasks(&self, project_id: &str) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            SELECT * FROM tasks
            WHERE project_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch tasks")?;

        Ok(tasks)
    }

    pub async fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks SET status = ? WHERE id = ?
            "#,
        )
        .bind(status.to_string())
        .bind(task_id)
        .execute(&self.pool)
        .await
        .context("Failed to update task status")?;

        Ok(())
    }

    pub async fn update_task(
        &self,
        task_id: &str,
        title: String,
        description: Option<String>,
        status: TaskStatus,
    ) -> Result<Task> {
        sqlx::query(
            r#"
            UPDATE tasks SET title = ?, description = ?, status = ? WHERE id = ?
            "#,
        )
        .bind(&title)
        .bind(&description)
        .bind(status.to_string())
        .bind(task_id)
        .execute(&self.pool)
        .await
        .context("Failed to update task")?;

        self.get_task(task_id).await
    }

    pub async fn delete_task(&self, task_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM tasks WHERE id = ?
            "#,
        )
        .bind(task_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete task")?;

        Ok(())
    }

    pub async fn start_session(
        &self,
        task_id: &str,
        prompt: &str,
        branch_name: Option<&str>,
    ) -> Result<Session> {
        let task = self.get_task(task_id).await?;
        let project = self.get_project(&task.project_id).await?;

        let session = self
            .session_manager
            .create_session(
                task_id,
                &project.path,
                prompt,
                self.executor.as_ref(),
                branch_name,
            )
            .await
            .context("Failed to create session")?;

        Ok(session)
    }

    pub async fn stop_session(&self, session_id: &str) -> Result<()> {
        self.session_manager
            .stop_session(session_id)
            .await
            .context("Failed to stop session")?;

        Ok(())
    }

    pub async fn cleanup_session(&self, session_id: &str, delete_branch: bool) -> Result<()> {
        self.session_manager
            .cleanup_session(session_id, delete_branch)
            .await
            .context("Failed to cleanup session")?;

        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Session> {
        self.session_manager
            .get_session(session_id)
            .await
    }

    pub async fn list_sessions(&self, task_id: &str) -> Result<Vec<Session>> {
        self.session_manager
            .list_sessions(task_id)
            .await
    }

    pub async fn get_logs(&self, session_id: &str) -> Result<Vec<crate::db::models::LogEntry>> {
        self.session_manager
            .get_logs(session_id)
            .await
    }

    pub async fn subscribe_logs(
        &self,
        session_id: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<LogMsg>> {
        self.session_manager
            .subscribe_logs(session_id)
            .await
    }

    pub async fn save_log_entry(
        &self,
        session_id: &str,
        log_type: LogType,
        content: &str,
    ) -> Result<()> {
        self.session_manager
            .save_log_entry(session_id, log_type, content)
            .await
            .context("Failed to save log entry")?;

        Ok(())
    }
}
