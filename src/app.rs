use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::connection::create_pool;
use crate::db::models::{LogEntry, LogType, Project, Session, SessionStatus, Task, TaskStatus};
use crate::executor::{Executor, LogMsg, OpenCodeExecutor};
use crate::session::SessionManager;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::ui::{ProjectView, SessionView, TaskView};
use crate::keyboard::{KeyboardState, Action, Direction, ViewLevel};
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

pub struct KanbanApp {
    project_view: ProjectView,
    task_view: TaskView,
    session_view: SessionView,
    projects: Arc<RwLock<Vec<Project>>>,
    tasks: Arc<RwLock<Vec<Task>>>,
    sessions: Arc<RwLock<Vec<Session>>>,
    current_session: Arc<RwLock<Option<Session>>>,
    logs: Arc<RwLock<Vec<LogEntry>>>,
    selected_project: Arc<RwLock<Option<String>>>,
    selected_task: Arc<RwLock<Option<String>>>,
    keyboard_state: KeyboardState,
    app_state: Arc<AppState>,
    show_create_project_dialog: bool,
    show_create_task_dialog: bool,
    show_edit_task_dialog: bool,
    show_help_dialog: bool,
    project_name_input: String,
    project_path_input: String,
    task_title_input: String,
    task_description_input: String,
}

impl KanbanApp {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let app = Self {
            project_view: ProjectView::new(),
            task_view: TaskView::new(),
            session_view: SessionView::new(),
            projects: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(Vec::new())),
            sessions: Arc::new(RwLock::new(Vec::new())),
            current_session: Arc::new(RwLock::new(None)),
            logs: Arc::new(RwLock::new(Vec::new())),
            selected_project: Arc::new(RwLock::new(None)),
            selected_task: Arc::new(RwLock::new(None)),
            keyboard_state: KeyboardState::new(),
            app_state: app_state.clone(),
            show_create_project_dialog: false,
            show_create_task_dialog: false,
            show_edit_task_dialog: false,
            show_help_dialog: false,
            project_name_input: String::new(),
            project_path_input: String::new(),
            task_title_input: String::new(),
            task_description_input: String::new(),
        };

        // Load initial data
        let projects = app.projects.clone();
        let state = app.app_state.clone();
        tokio::spawn(async move {
            if let Ok(p) = state.list_projects().await {
                *projects.write().await = p;
            }
        });

        app
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

    pub async fn set_projects(&self, projects: Vec<Project>) {
        *self.projects.write().await = projects;
    }

    pub async fn set_selected_task(&self, task_id: Option<String>) {
        *self.selected_task.write().await = task_id;
    }

    pub async fn set_sessions(&self, sessions: Vec<Session>) {
        *self.sessions.write().await = sessions;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.handle_keyboard_input(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("iKanban");
                ui.separator();
                ui.label(
                    egui::RichText::new(self.keyboard_state.get_view_string())
                        .color(egui::Color32::from_rgb(100, 200, 150))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match self.keyboard_state.view_level {
                        ViewLevel::Project => {
                            let projects = self.projects.blocking_read();
                            ui.label(format!(
                                "Project: {}/{}",
                                self.keyboard_state.selected_project_index + 1,
                                projects.len().max(1)
                            ));
                        }
                        ViewLevel::Task => {
                            ui.label(format!(
                                "Col: {} Row: {}",
                                self.keyboard_state.selected_column + 1,
                                self.keyboard_state.selected_row + 1
                            ));
                        }
                        ViewLevel::Session => {
                            let sessions = self.sessions.blocking_read();
                            ui.label(format!(
                                "Session: {}/{}",
                                self.keyboard_state.selected_session_index + 1,
                                sessions.len().max(1)
                            ));
                        }
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let help_text = match self.keyboard_state.view_level {
                        ViewLevel::Project => {
                            "j/k - navigate | Enter - open project | n - new | dd - delete | q - quit"
                        }
                        ViewLevel::Task => {
                            "h/j/k/l - navigate | Enter - open task | n - new | e - edit | dd - delete | 1-4 - columns | Esc - back"
                        }
                        ViewLevel::Session => {
                            "j/k - sessions | s - start | x - stop | Esc - back"
                        }
                    };
                    ui.label(help_text);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.keyboard_state.view_level {
                ViewLevel::Project => {
                    self.show_project_view(ui);
                }
                ViewLevel::Task => {
                    self.show_task_view(ui);
                }
                ViewLevel::Session => {
                    self.show_session_view(ui);
                }
            }
        });

        self.show_create_project_dialog(ctx);
        self.show_create_task_dialog(ctx);
        self.show_edit_task_dialog(ctx);
        self.show_help_dialog(ctx);
    }

    fn show_project_view(&mut self, ui: &mut egui::Ui) {
        let projects = self.projects.blocking_read();
        if let Some(project_id) = self.project_view.show(ui, &projects, &self.keyboard_state) {
            drop(projects);
            *self.selected_project.blocking_write() = Some(project_id);
            self.keyboard_state.drill_down();
        }
    }

    fn show_task_view(&mut self, ui: &mut egui::Ui) {
        let projects = self.projects.blocking_read();
        let selected_project_id = self.selected_project.blocking_read();
        let project = selected_project_id
            .as_ref()
            .and_then(|id| projects.iter().find(|p| &p.id == id));

        let tasks = self.tasks.blocking_read();
        if let Some(task_id) = self.task_view.show(ui, project, &tasks, &self.keyboard_state) {
            drop(tasks);
            drop(projects);
            drop(selected_project_id);
            *self.selected_task.blocking_write() = Some(task_id);
        }
    }

    fn show_session_view(&mut self, ui: &mut egui::Ui) {
        let selected_task_id = self.selected_task.blocking_read();
        let tasks = self.tasks.blocking_read();
        let task = selected_task_id
            .as_ref()
            .and_then(|id| tasks.iter().find(|t| &t.id == id));

        let sessions = self.sessions.blocking_read();
        let current_session = self.current_session.blocking_read();
        let logs = self.logs.blocking_read();

        let _action = self.session_view.show(
            ui,
            task,
            &sessions,
            current_session.as_ref(),
            &logs,
            &self.keyboard_state,
        );
    }

    fn show_create_project_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_create_project_dialog {
            return;
        }

        egui::Window::new("Create New Project")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Project Name:");
                    ui.text_edit_singleline(&mut self.project_name_input);
                    
                    ui.add_space(10.0);
                    
                    ui.label("Project Path:");
                    ui.text_edit_singleline(&mut self.project_path_input);
                    
                    ui.add_space(15.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            self.handle_save();
                        }
                        
                        if ui.button("Cancel").clicked() {
                            self.show_create_project_dialog = false;
                            self.project_name_input.clear();
                            self.project_path_input.clear();
                        }
                    });
                });
            });
    }

    fn show_create_task_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_create_task_dialog {
            return;
        }

        egui::Window::new("Create New Task")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Task Title:");
                    ui.text_edit_singleline(&mut self.task_title_input);
                    
                    ui.add_space(10.0);
                    
                    ui.label("Description (optional):");
                    ui.text_edit_multiline(&mut self.task_description_input);
                    
                    ui.add_space(15.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            self.handle_save();
                        }
                        
                        if ui.button("Cancel").clicked() {
                            self.show_create_task_dialog = false;
                            self.task_title_input.clear();
                            self.task_description_input.clear();
                        }
                    });
                });
            });
    }

    fn prepare_edit_task(&mut self) {
        let tasks = self.tasks.blocking_read();
        if let Some(task) = self.task_view.get_selected_task(&tasks, &self.keyboard_state) {
            self.task_title_input = task.title.clone();
            self.task_description_input = task.description.clone().unwrap_or_default();
            self.show_edit_task_dialog = true;
        }
    }

    fn handle_save(&mut self) {
        if self.show_create_project_dialog {
            self.create_project_internal();
            self.show_create_project_dialog = false;
            self.project_name_input.clear();
            self.project_path_input.clear();
        } else if self.show_create_task_dialog {
            self.create_task_internal();
            self.show_create_task_dialog = false;
            self.task_title_input.clear();
            self.task_description_input.clear();
        } else if self.show_edit_task_dialog {
            self.update_task_internal();
            self.show_edit_task_dialog = false;
            self.task_title_input.clear();
            self.task_description_input.clear();
        }
    }

    fn create_project_internal(&mut self) {
        let name = self.project_name_input.trim().to_string();
        let path = PathBuf::from(self.project_path_input.trim());
        if name.is_empty() {
            return;
        }

        let app_state = self.app_state.clone();
        let projects = self.projects.clone();

        tokio::spawn(async move {
            match app_state.create_project(name, path).await {
                Ok(project) => {
                    projects.write().await.push(project);
                }
                Err(e) => {
                    eprintln!("Failed to create project: {}", e);
                }
            }
        });
    }

    fn create_task_internal(&mut self) {
        let title = self.task_title_input.trim().to_string();
        let description = self.task_description_input.trim().to_string();
        if title.is_empty() {
            return;
        }

        let project_id_guard = self.selected_project.blocking_read();
        let project_id = match project_id_guard.clone() {
            Some(id) => id,
            None => return,
        };
        drop(project_id_guard);

        let app_state = self.app_state.clone();
        let tasks = self.tasks.clone();
        let description = if description.is_empty() { None } else { Some(description) };

        tokio::spawn(async move {
            match app_state.create_task(project_id, title, description).await {
                Ok(task) => {
                    tasks.write().await.push(task);
                }
                Err(e) => {
                    eprintln!("Failed to create task: {}", e);
                }
            }
        });
    }

    fn update_task_internal(&mut self) {
        let title = self.task_title_input.trim().to_string();
        let description = self.task_description_input.trim().to_string();
        if title.is_empty() {
            return;
        }

        let tasks_guard = self.tasks.blocking_read();
        let selected_task = self.task_view.get_selected_task(&tasks_guard, &self.keyboard_state).cloned();
        drop(tasks_guard);

        if let Some(task) = selected_task {
            let app_state = self.app_state.clone();
            let tasks = self.tasks.clone();
            let description = if description.is_empty() { None } else { Some(description) };
            let status = task.status;

            tokio::spawn(async move {
                match app_state.update_task(&task.id, title, description, status).await {
                    Ok(updated_task) => {
                        let mut tasks_write = tasks.write().await;
                        if let Some(t) = tasks_write.iter_mut().find(|t| t.id == updated_task.id) {
                            *t = updated_task;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to update task: {}", e);
                    }
                }
            });
        }
    }

    fn show_edit_task_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_edit_task_dialog {
            return;
        }

        egui::Window::new("Edit Task")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Task Title:");
                    ui.text_edit_singleline(&mut self.task_title_input);
                    
                    ui.add_space(10.0);
                    
                    ui.label("Description (optional):");
                    ui.text_edit_multiline(&mut self.task_description_input);
                    
                    ui.add_space(15.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.handle_save();
                        }
                        
                        if ui.button("Cancel").clicked() {
                            self.show_edit_task_dialog = false;
                            self.task_title_input.clear();
                            self.task_description_input.clear();
                        }
                    });
                });
            });
    }

    fn show_help_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_help_dialog {
            return;
        }

        egui::Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading(match self.keyboard_state.view_level {
                        ViewLevel::Project => "Project View Shortcuts",
                        ViewLevel::Task => "Task View Shortcuts",
                        ViewLevel::Session => "Session View Shortcuts",
                    });
                    ui.add_space(10.0);

                    match self.keyboard_state.view_level {
                        ViewLevel::Project => {
                            ui.label("Navigation:");
                            ui.label("  j/k - Move up/down");
                            ui.label("  Enter - Open project");
                            ui.add_space(5.0);
                            ui.label("Actions:");
                            ui.label("  n - Create new project");
                            ui.label("  dd - Delete project");
                            ui.label("  ? - Toggle this help");
                            ui.add_space(5.0);
                            ui.label("Esc - Cancel / Go back");
                        }
                        ViewLevel::Task => {
                            ui.label("Navigation:");
                            ui.label("  h/j/k/l - Move left/down/up/right");
                            ui.label("  Enter - Open task");
                            ui.label("  1-4 - Jump to column");
                            ui.add_space(5.0);
                            ui.label("Actions:");
                            ui.label("  n - Create new task");
                            ui.label("  e - Edit task");
                            ui.label("  dd - Delete task");
                            ui.label("  ? - Toggle this help");
                            ui.add_space(5.0);
                            ui.label("Esc - Cancel / Go back");
                        }
                        ViewLevel::Session => {
                            ui.label("Navigation:");
                            ui.label("  j/k - Move up/down");
                            ui.add_space(5.0);
                            ui.label("Actions:");
                            ui.label("  s - Start new session");
                            ui.label("  x - Stop session");
                            ui.label("  ? - Toggle this help");
                            ui.add_space(5.0);
                            ui.label("Esc - Cancel / Go back");
                        }
                    }

                    ui.add_space(15.0);
                    ui.label("Press ? or Esc to close");
                });
            });
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        let wants_input = ctx.wants_keyboard_input();
        ctx.input(|i| {
            let modifiers = i.modifiers;
            
            for event in &i.events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    if wants_input {
                        if modifiers.ctrl {
                            if *key != egui::Key::S {
                                continue;
                            }
                        } else if *key != egui::Key::Escape {
                            continue;
                        }
                    }

                    let action = self.keyboard_state.handle_key(*key, &modifiers);
                    self.execute_action(action);
                }
            }
        });
    }

    fn execute_action(&mut self, action: Action) {
        match action {
            Action::MoveSelection(direction) => {
                self.handle_move_selection(direction);
            }
            Action::JumpToTop => {
                self.keyboard_state.jump_to_top();
            }
            Action::JumpToBottom => {
                let tasks = self.tasks.blocking_read();
                let column_sizes = self.get_column_sizes(&tasks);
                let column_size = column_sizes[self.keyboard_state.selected_column];
                self.keyboard_state.jump_to_bottom(column_size);
            }
            Action::JumpToColumn(col) => {
                if self.keyboard_state.view_level == ViewLevel::Task {
                    let tasks = self.tasks.blocking_read();
                    let column_sizes = self.get_column_sizes(&tasks);
                    self.keyboard_state.jump_to_column(col, 4, &column_sizes);
                }
            }
            Action::DrillDown => {
                self.handle_drill_down();
            }
            Action::GoBack => {
                if self.show_help_dialog {
                    self.show_help_dialog = false;
                } else {
                    self.keyboard_state.go_back();
                }
            }
            Action::Quit => {
                std::process::exit(0);
            }
            Action::CreateProject => {
                self.show_create_project_dialog = true;
            }
            Action::CreateTask => {
                self.show_create_task_dialog = true;
            }
            Action::EditTask => {
                self.prepare_edit_task();
            }
            Action::Save => {
                self.handle_save();
            }
            Action::ToggleHelp => {
                self.show_help_dialog = !self.show_help_dialog;
            }
            _ => {}
        }
    }

    fn handle_move_selection(&mut self, direction: Direction) {
        match self.keyboard_state.view_level {
            ViewLevel::Project => {
                let projects = self.projects.blocking_read();
                self.keyboard_state
                    .move_project_selection(direction, projects.len());
            }
            ViewLevel::Task => {
                let tasks = self.tasks.blocking_read();
                let column_sizes = self.get_column_sizes(&tasks);
                self.keyboard_state.move_selection(direction, 4, &column_sizes);
            }
            ViewLevel::Session => {
                let sessions = self.sessions.blocking_read();
                self.keyboard_state
                    .move_session_selection(direction, sessions.len());
            }
        }
    }

    fn handle_drill_down(&mut self) {
        match self.keyboard_state.view_level {
            ViewLevel::Project => {
                let projects = self.projects.blocking_read();
                if let Some(project) = projects.get(self.keyboard_state.selected_project_index) {
                    let project_id = project.id.clone();
                    drop(projects);
                    *self.selected_project.blocking_write() = Some(project_id);
                    self.keyboard_state.drill_down();
                }
            }
            ViewLevel::Task => {
                let tasks = self.tasks.blocking_read();
                if let Some(task) = self.task_view.get_selected_task(&tasks, &self.keyboard_state) {
                    let task_id = task.id.clone();
                    drop(tasks);
                    *self.selected_task.blocking_write() = Some(task_id);
                    self.keyboard_state.drill_down();
                }
            }
            ViewLevel::Session => {}
        }
    }

    fn get_column_sizes(&self, tasks: &[Task]) -> Vec<usize> {
        let mut sizes = vec![0; 4];
        for task in tasks {
            let idx = match task.status {
                TaskStatus::Todo => 0,
                TaskStatus::InProgress => 1,
                TaskStatus::InReview => 2,
                TaskStatus::Done => 3,
            };
            sizes[idx] += 1;
        }
        sizes.iter().map(|&s| s.max(1)).collect()
    }
}

impl Default for KanbanApp {
    fn default() -> Self {
        panic!("KanbanApp::default() is not supported. Use KanbanApp::new(app_state) instead.");
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
