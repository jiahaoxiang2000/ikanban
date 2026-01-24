use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::connection::create_pool;
use crate::db::models::{LogEntry, LogType, Project, Session, Task, TaskStatus};
use crate::executor::{Executor, LogMsg, OpenCodeExecutor};
use crate::session::SessionManager;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::ui::{Board, SessionPanel};
use eframe;
use egui;

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

pub struct KanbanApp {
    board: Board,
    session_panel: SessionPanel,
    tasks: Arc<RwLock<Vec<Task>>>,
    current_session: Arc<RwLock<Option<Session>>>,
    logs: Arc<RwLock<Vec<LogEntry>>>,
    selected_project: Arc<RwLock<Option<String>>>,
}

impl KanbanApp {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
            session_panel: SessionPanel::new(),
            tasks: Arc::new(RwLock::new(Vec::new())),
            current_session: Arc::new(RwLock::new(None)),
            logs: Arc::new(RwLock::new(Vec::new())),
            selected_project: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_tasks(&self, tasks: Vec<Task>) {
        *self.tasks.write().await = tasks;
    }

    pub async fn set_session(&self, session: Option<Session>) {
        *self.current_session.write().await = session;
    }

    pub async fn set_logs(&self, logs: Vec<LogEntry>) {
        *self.logs.write().await = logs;
    }

    pub async fn set_project(&self, project_id: Option<String>) {
        *self.selected_project.write().await = project_id;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("iKanban - AI-Powered Task Management");

            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(800.0);

                    let tasks = self.tasks.blocking_read();
                    self.board.show(ui, &tasks);
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.set_min_width(400.0);

                    let session = self.current_session.blocking_read();
                    let logs = self.logs.blocking_read();
                    self.session_panel.show(ui, session.as_ref(), &logs);
                });
            });
        });
    }
}

impl Default for KanbanApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for KanbanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_state_creation() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        let app_state = AppState::new(db_path).await.unwrap();
        let projects = app_state.list_projects().await.unwrap();
        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn test_project_crud() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let app_state = AppState::new(db_path).await.unwrap();

        let project = app_state
            .create_project("Test Project".to_string(), dir.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(project.name, "Test Project");

        let fetched = app_state.get_project(&project.id).await.unwrap();
        assert_eq!(fetched.id, project.id);

        let projects = app_state.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);

        app_state.delete_project(&project.id).await.unwrap();
        let projects = app_state.list_projects().await.unwrap();
        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn test_task_crud() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let app_state = AppState::new(db_path).await.unwrap();

        let project = app_state
            .create_project("Test Project".to_string(), dir.path().to_path_buf())
            .await
            .unwrap();

        let task = app_state
            .create_task(
                project.id.clone(),
                "Test Task".to_string(),
                Some("Description".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::Todo);

        let fetched = app_state.get_task(&task.id).await.unwrap();
        assert_eq!(fetched.id, task.id);

        app_state
            .update_task_status(&task.id, TaskStatus::InProgress)
            .await
            .unwrap();

        let updated = app_state.get_task(&task.id).await.unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);

        let tasks = app_state.list_tasks(&project.id).await.unwrap();
        assert_eq!(tasks.len(), 1);

        app_state.delete_task(&task.id).await.unwrap();
        let tasks = app_state.list_tasks(&project.id).await.unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_kanban_app_creation() {
        let app = KanbanApp::new();
        assert!(app.tasks.blocking_read().is_empty());
    }
}
