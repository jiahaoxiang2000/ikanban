use tokio::sync::mpsc;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::models::{
    CreateProject, CreateTask, ExecutionProcess, ExecutionProcessLog, Project, Task, TaskStatus,
    TaskStatusExt, UpdateTask, WsEvent,
};
use crate::ws::WebSocketClient;

/// Current view/screen in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Projects,
    ProjectDetail,
    Tasks,
    TaskDetail,
    ExecutionLogs,
}

/// Input mode for text entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

/// Application state
pub struct App {
    pub api: ApiClient,
    pub view: View,
    pub input_mode: InputMode,
    pub input: String,
    pub input_field: InputField,
    pub input_cursor_row: usize,
    pub input_cursor_col: usize,

    // Projects
    pub projects: Vec<Project>,
    pub selected_project_index: usize,
    pub project_detail: Option<Project>,

    // Tasks
    pub tasks: Vec<Task>,
    pub selected_task_index: usize,
    pub selected_column: TaskStatus,
    pub task_detail: Option<Task>,

    // Status message
    pub status_message: Option<String>,

    // Running flag
    pub running: bool,

    // WebSocket state
    pub ws_event_rx: Option<mpsc::UnboundedReceiver<WsEvent>>,
    ws_event_tx: Option<mpsc::UnboundedSender<WsEvent>>,
    pub projects_ws: Option<WebSocketClient>,
    pub tasks_ws: Option<WebSocketClient>,
    pub current_project_id: Option<Uuid>,

    // Help modal state
    pub show_help_modal: bool,
    pub help_modal_selected: usize,

    // Execution logs state
    pub executions: Vec<ExecutionProcess>,
    pub selected_execution_index: usize,
    pub current_execution_logs: Vec<ExecutionProcessLog>,
    pub log_view_line_offset: usize,
    pub current_session_id: Option<Uuid>,
    pub execution_logs_ws: Option<WebSocketClient>,
}

/// Which field is being edited
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    None,
    ProjectName,
    ProjectDescription,
    ProjectRepoPath,
    TaskTitle,
    TaskDescription,
}

impl App {
    pub fn new(server_url: &str) -> Self {
        // Create channel for WebSocket events
        let (ws_event_tx, ws_event_rx) = mpsc::unbounded_channel::<WsEvent>();

        Self {
            api: ApiClient::new(server_url),
            view: View::Projects,
            input_mode: InputMode::Normal,
            input: String::new(),
            input_field: InputField::None,
            input_cursor_row: 0,
            input_cursor_col: 0,
            projects: Vec::new(),
            selected_project_index: 0,
            project_detail: None,
            tasks: Vec::new(),
            selected_task_index: 0,
            selected_column: TaskStatus::Todo,
            task_detail: None,
            status_message: None,
            running: true,
            ws_event_rx: Some(ws_event_rx),
            ws_event_tx: Some(ws_event_tx),
            projects_ws: None,
            tasks_ws: None,
            current_project_id: None,
            show_help_modal: false,
            help_modal_selected: 0,
            executions: Vec::new(),
            selected_execution_index: 0,
            current_execution_logs: Vec::new(),
            log_view_line_offset: 0,
            current_session_id: None,
            execution_logs_ws: None,
        }
    }

    pub fn set_status(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    // Help modal methods

    pub fn toggle_help_modal(&mut self) {
        self.show_help_modal = !self.show_help_modal;
        self.help_modal_selected = 0;
    }

    pub fn close_help_modal(&mut self) {
        self.show_help_modal = false;
    }

    /// Get the keyboard shortcuts for the current view (major keys only)
    pub fn get_keyboard_shortcuts(&self) -> Vec<(String, String)> {
        match self.view {
            View::Projects => vec![
                ("n".to_string(), "New project".to_string()),
                ("e".to_string(), "Edit project".to_string()),
                ("d".to_string(), "Delete project".to_string()),
                ("j / k".to_string(), "Navigate projects".to_string()),
                ("Enter".to_string(), "Open tasks".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("q".to_string(), "Quit".to_string()),
            ],
            View::ProjectDetail => vec![
                ("Enter".to_string(), "Open tasks".to_string()),
                ("e / d / r".to_string(), "Edit fields".to_string()),
                ("j / k".to_string(), "Navigate".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("Esc".to_string(), "Go back".to_string()),
            ],
            View::Tasks => vec![
                ("h / l".to_string(), "Switch column".to_string()),
                ("j / k".to_string(), "Navigate tasks".to_string()),
                ("Space".to_string(), "Move status".to_string()),
                ("n".to_string(), "New task".to_string()),
                ("Enter".to_string(), "Task details".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("Esc".to_string(), "Go back".to_string()),
            ],
            View::TaskDetail => vec![
                ("e / d".to_string(), "Edit fields".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("Esc".to_string(), "Go back".to_string()),
            ],
            View::ExecutionLogs => vec![
                ("j / k".to_string(), "Navigate logs".to_string()),
                ("g / G".to_string(), "Go to top/bottom".to_string()),
                ("Enter".to_string(), "View execution details".to_string()),
                ("s".to_string(), "Stop execution".to_string()),
                ("r".to_string(), "Refresh logs".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("Esc".to_string(), "Go back".to_string()),
            ],
        }
    }

    /// Get keyboard shortcuts title for current view
    pub fn get_keyboard_shortcuts_title(&self) -> String {
        match self.view {
            View::Projects => "Projects View Shortcuts",
            View::ProjectDetail => "Project Detail Shortcuts",
            View::Tasks => "Tasks View Shortcuts",
            View::TaskDetail => "Task Detail Shortcuts",
            View::ExecutionLogs => "Execution Logs Shortcuts",
        }
        .to_string()
    }

    /// Compare two TaskStatus values for sorting
    fn compare_task_status(a: &TaskStatus, b: &TaskStatus) -> std::cmp::Ordering {
        fn order(status: &TaskStatus) -> u8 {
            match status {
                TaskStatus::Todo => 0,
                TaskStatus::InProgress => 1,
                TaskStatus::InReview => 2,
                TaskStatus::Done => 3,
                TaskStatus::Cancelled => 4,
            }
        }
        order(a).cmp(&order(b))
    }

    /// Connect to projects WebSocket stream
    pub fn connect_projects_ws(&mut self) {
        let event_tx = self
            .ws_event_tx()
            .expect("WebSocket event channel should be available");
        self.projects_ws = Some(WebSocketClient::projects(&self.api.base_url(), event_tx));
        self.set_status("Connected to projects stream");
    }

    /// Connect to tasks WebSocket stream for a specific project
    pub fn connect_tasks_ws(&mut self, project_id: Uuid) {
        // Disconnect from previous tasks stream if any
        self.tasks_ws = None;

        let event_tx = self
            .ws_event_tx()
            .expect("WebSocket event channel should be available");
        self.tasks_ws = Some(WebSocketClient::tasks(
            &self.api.base_url(),
            project_id,
            event_tx,
        ));
        self.current_project_id = Some(project_id);
        self.set_status(&format!(
            "Connected to tasks stream for project {}",
            project_id
        ));
    }

    /// Disconnect from tasks WebSocket stream
    pub fn disconnect_tasks_ws(&mut self) {
        self.tasks_ws = None;
        self.current_project_id = None;
    }

    /// Connect to execution logs WebSocket stream
    pub fn connect_execution_logs_ws(&mut self, execution_id: Uuid) {
        let event_tx = self
            .ws_event_tx()
            .expect("WebSocket event channel should be available");
        self.execution_logs_ws = Some(WebSocketClient::execution_logs(
            &self.api.base_url(),
            execution_id,
            event_tx,
        ));
        self.set_status(&format!("Connected to logs stream for execution {}", execution_id));
    }

    /// Disconnect from execution logs WebSocket stream
    pub fn disconnect_execution_logs_ws(&mut self) {
        self.execution_logs_ws = None;
    }

    /// Load executions for a session
    pub async fn load_executions(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        self.current_session_id = Some(session_id);
        self.executions = self.api.list_executions(session_id).await?;
        self.selected_execution_index = 0;
        Ok(())
    }

    /// Load logs for the selected execution
    pub async fn load_execution_logs(&mut self) -> anyhow::Result<()> {
        if let Some(execution) = self.selected_execution() {
            let execution_id = execution.id;
            self.current_execution_logs = self.api.get_execution_logs(execution_id, None).await?;
            self.connect_execution_logs_ws(execution_id);
        }
        Ok(())
    }

    /// Stop the selected execution
    pub async fn stop_selected_execution(&mut self) -> anyhow::Result<()> {
        if let Some(execution) = self.selected_execution() {
            let execution_id = execution.id;
            let session_id = execution.session_id;
            self.api.stop_execution(execution_id).await?;
            self.set_status(&format!("Stopped execution {}", execution_id));
            self.load_executions(session_id).await?;
        }
        Ok(())
    }

    /// Get the currently selected execution
    pub fn selected_execution(&self) -> Option<&ExecutionProcess> {
        self.executions.get(self.selected_execution_index)
    }

    /// Navigation methods for execution logs
    pub fn next_log_line(&mut self) {
        let max_offset = self.current_execution_logs.len().saturating_sub(1);
        if self.log_view_line_offset < max_offset {
            self.log_view_line_offset += 1;
        }
    }

    pub fn previous_log_line(&mut self) {
        if self.log_view_line_offset > 0 {
            self.log_view_line_offset -= 1;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.log_view_line_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.log_view_line_offset = self.current_execution_logs.len().saturating_sub(1);
    }

    pub fn next_execution(&mut self) {
        if !self.executions.is_empty() {
            self.selected_execution_index = (self.selected_execution_index + 1) % self.executions.len();
        }
    }

    pub fn previous_execution(&mut self) {
        if !self.executions.is_empty() {
            self.selected_execution_index = if self.selected_execution_index == 0 {
                self.executions.len() - 1
            } else {
                self.selected_execution_index - 1
            };
        }
    }

    /// Enter execution logs view
    pub async fn enter_execution_logs_view(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        self.view = View::ExecutionLogs;
        self.log_view_line_offset = 0;
        self.load_executions(session_id).await?;
        if !self.executions.is_empty() {
            self.load_execution_logs().await?;
        }
        Ok(())
    }

    /// Leave execution logs view
    pub fn leave_execution_logs_view(&mut self) {
        self.disconnect_execution_logs_ws();
        self.view = View::TaskDetail;
        self.current_session_id = None;
        self.executions = Vec::new();
        self.current_execution_logs = Vec::new();
    }

    /// Get the event transmitter for WebSocket clients
    fn ws_event_tx(&self) -> Option<mpsc::UnboundedSender<WsEvent>> {
        self.ws_event_tx.clone()
    }

    /// Process incoming WebSocket events
    pub async fn process_ws_events(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut rx) = self.ws_event_rx {
            // Collect all available events first to avoid borrow issues
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            // Then process each event
            for event in events {
                self.handle_ws_event(event).await;
            }
        }
        Ok(())
    }

    /// Handle a single WebSocket event
    pub async fn handle_ws_event(&mut self, event: WsEvent) {
        match event {
            WsEvent::ProjectCreated(project) => {
                self.projects.push(project);
                self.set_status("New project created");
            }
            WsEvent::ProjectUpdated(project) => {
                if let Some(existing) = self.projects.iter_mut().find(|p| p.id == project.id) {
                    *existing = project.clone();
                }
                if let Some(ref mut detail) = self.project_detail {
                    if detail.id == project.id {
                        *detail = project;
                    }
                }
                self.set_status("Project updated");
            }
            WsEvent::ProjectDeleted { id } => {
                self.projects.retain(|p| p.id != id);
                if let Some(ref mut detail) = self.project_detail {
                    if detail.id == id {
                        self.project_detail = None;
                    }
                }
                if self.selected_project_index >= self.projects.len() {
                    self.selected_project_index = self.projects.len().saturating_sub(1);
                }
                self.set_status("Project deleted");
            }
            WsEvent::TaskCreated(task) => {
                // Only add if it's for the current project
                if Some(task.project_id) == self.current_project_id {
                    self.tasks.push(task);
                    // Sort tasks by status for proper display
                    self.tasks
                        .sort_by(|a, b| Self::compare_task_status(&a.status, &b.status));
                    self.set_status("New task created");
                }
            }
            WsEvent::TaskUpdated(task) => {
                if Some(task.project_id) == self.current_project_id {
                    if let Some(existing) = self.tasks.iter_mut().find(|t| t.id == task.id) {
                        *existing = task.clone();
                    }
                    // Re-sort tasks by status
                    self.tasks
                        .sort_by(|a, b| Self::compare_task_status(&a.status, &b.status));
                    self.set_status("Task updated");
                }
            }
            WsEvent::TaskDeleted { id } => {
                if let Some(project_id) = self.current_project_id {
                    // We can't easily filter by project_id here, but the server
                    // should only send delete events for the current project
                    self.tasks.retain(|t| t.id != id);
                    if self.selected_task_index >= self.tasks.len() {
                        self.selected_task_index = self.tasks.len().saturating_sub(1);
                    }
                    self.set_status("Task deleted");
                }
            }
            WsEvent::Connected => {
                self.set_status("WebSocket connected");
            }
            WsEvent::Log {
                execution_id,
                content,
            } => {
                // Add log to the current execution logs if viewing that execution
                if self.view == View::ExecutionLogs {
                    if let Some(execution) = self.selected_execution() {
                        if execution.id == execution_id {
                            // Create a new log entry from the streamed content
                            let log = ExecutionProcessLog {
                                id: Uuid::new_v4(),
                                execution_process_id: execution_id,
                                level: "info".to_string(),
                                message: content.clone(),
                                timestamp: chrono::Utc::now().naive_utc(),
                                created_at: chrono::Utc::now().naive_utc(),
                            };
                            self.current_execution_logs.push(log);
                            // Auto-scroll to bottom if already at bottom
                            if self.log_view_line_offset >= self.current_execution_logs.len().saturating_sub(2) {
                                self.scroll_to_bottom();
                            }
                        }
                    }
                }
                tracing::info!("Log: {}", content);
            }
            WsEvent::SessionCreated(session) => {
                tracing::debug!("Session created: {}", session.id);
            }
            WsEvent::SessionUpdated(session) => {
                tracing::debug!("Session updated: {}", session.id);
            }
            WsEvent::ExecutionCreated(execution) => {
                tracing::debug!("Execution created: {}", execution.id);
                // If we're viewing executions for this session, reload
                if self.view == View::ExecutionLogs && self.current_session_id == Some(execution.session_id) {
                    let session_id = execution.session_id;
                    let _ = self.load_executions(session_id);
                }
            }
            WsEvent::ExecutionUpdated(execution) => {
                tracing::debug!("Execution updated: {}", execution.id);
                // Update the execution in our list
                if let Some(existing) = self.executions.iter_mut().find(|e| e.id == execution.id) {
                    *existing = execution.clone();
                }
            }
            WsEvent::DirectMergeCreated(_) => tracing::debug!("Direct merge created"),
            WsEvent::PrMergeCreated(_) => tracing::debug!("PR merge created"),
            WsEvent::PrMergeUpdated(_) => tracing::debug!("PR merge updated"),
            WsEvent::Ping => {
                // Handle ping if needed
            }
            WsEvent::Pong => {
                // Handle pong if needed
            }
        }
    }

    pub fn selected_project(&self) -> Option<&Project> {
        self.projects.get(self.selected_project_index)
    }

    pub fn selected_task(&self) -> Option<&Task> {
        let tasks_in_column: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| t.status == self.selected_column)
            .collect();
        tasks_in_column.get(self.selected_task_index).copied()
    }

    pub fn tasks_in_column(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == status).collect()
    }

    // Navigation

    pub fn next_project(&mut self) {
        if !self.projects.is_empty() {
            self.selected_project_index = (self.selected_project_index + 1) % self.projects.len();
        }
    }

    pub fn previous_project(&mut self) {
        if !self.projects.is_empty() {
            self.selected_project_index = if self.selected_project_index == 0 {
                self.projects.len() - 1
            } else {
                self.selected_project_index - 1
            };
        }
    }

    pub fn next_task(&mut self) {
        let count = self.tasks_in_column(self.selected_column).len();
        if count > 0 {
            self.selected_task_index = (self.selected_task_index + 1) % count;
        }
    }

    pub fn previous_task(&mut self) {
        let count = self.tasks_in_column(self.selected_column).len();
        if count > 0 {
            self.selected_task_index = if self.selected_task_index == 0 {
                count - 1
            } else {
                self.selected_task_index - 1
            };
        }
    }

    pub fn next_column(&mut self) {
        self.selected_column = match self.selected_column {
            TaskStatus::Todo => TaskStatus::InProgress,
            TaskStatus::InProgress => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Todo,
            _ => TaskStatus::Todo,
        };
        self.selected_task_index = 0;
    }

    pub fn previous_column(&mut self) {
        self.selected_column = match self.selected_column {
            TaskStatus::Todo => TaskStatus::Done,
            TaskStatus::InProgress => TaskStatus::Todo,
            TaskStatus::Done => TaskStatus::InProgress,
            _ => TaskStatus::Todo,
        };
        self.selected_task_index = 0;
    }

    // API operations

    pub async fn load_projects(&mut self) -> anyhow::Result<()> {
        self.projects = self.api.list_projects().await?;
        if self.selected_project_index >= self.projects.len() {
            self.selected_project_index = self.projects.len().saturating_sub(1);
        }
        Ok(())
    }

    pub async fn load_tasks(&mut self, project_id: Uuid) -> anyhow::Result<()> {
        self.tasks = self.api.list_tasks(project_id).await?;
        self.selected_task_index = 0;
        Ok(())
    }

    pub async fn create_project(&mut self, name: String) -> anyhow::Result<()> {
        let payload = CreateProject {
            name,
            description: None,
            repo_path: None,
        };
        self.api.create_project(&payload).await?;
        // Don't reload - WebSocket event will add the new project
        Ok(())
    }

    pub async fn delete_selected_project(&mut self) -> anyhow::Result<()> {
        if let Some(project) = self.selected_project() {
            let id = project.id;
            self.api.delete_project(id).await?;
            self.load_projects().await?;
        }
        Ok(())
    }

    pub async fn update_project_detail(
        &mut self,
        name: Option<String>,
        description: Option<String>,
        repo_path: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(project) = &self.project_detail {
            use crate::models::UpdateProject;
            let payload = UpdateProject {
                name,
                description,
                repo_path,
                archived: None,
                pinned: None,
            };
            let updated = self.api.update_project(project.id, &payload).await?;
            self.project_detail = Some(updated);
            self.load_projects().await?;
        }
        Ok(())
    }

    pub async fn create_task(&mut self, title: String) -> anyhow::Result<()> {
        if let Some(project) = self.selected_project() {
            let payload = CreateTask {
                project_id: project.id,
                title,
                description: None,
                status: Some(self.selected_column),
                branch: None,
                working_dir: None,
                parent_task_id: None,
            };
            self.api.create_task(&payload).await?;
            // Don't reload - WebSocket event will add the new task
        }
        Ok(())
    }

    pub async fn delete_selected_task(&mut self) -> anyhow::Result<()> {
        if let Some(task) = self.selected_task() {
            let id = task.id;
            let project_id = task.project_id;
            self.api.delete_task(id).await?;
            self.load_tasks(project_id).await?;
        }
        Ok(())
    }

    pub async fn move_task_to_next_status(&mut self) -> anyhow::Result<()> {
        if let Some(task) = self.selected_task() {
            let id = task.id;
            let project_id = task.project_id;
            let new_status = task.status.next();

            let payload = UpdateTask {
                title: None,
                description: None,
                status: Some(new_status),
                branch: None,
                working_dir: None,
                parent_task_id: None,
            };
            self.api.update_task(id, &payload).await?;
            self.load_tasks(project_id).await?;
        }
        Ok(())
    }

    pub async fn update_task_detail(
        &mut self,
        title: Option<String>,
        description: Option<String>,
    ) -> anyhow::Result<()> {
        let task = if let Some(t) = &self.task_detail {
            t.clone()
        } else {
            return Ok(());
        };

        let payload = UpdateTask {
            title,
            description,
            status: None,
            branch: None,
            working_dir: None,
            parent_task_id: None,
        };
        let updated = self.api.update_task(task.id, &payload).await?;
        self.task_detail = Some(updated);
        self.load_tasks(task.project_id).await?;

        Ok(())
    }

    pub fn enter_project_view(&mut self) {
        self.view = View::Projects;
        self.selected_task_index = 0;
        self.selected_column = TaskStatus::Todo;
        self.project_detail = None;
        self.task_detail = None;
        // Disconnect from tasks WebSocket
        self.disconnect_tasks_ws();
        // Connect to projects WebSocket
        self.connect_projects_ws();
    }

    pub fn enter_project_detail_view(&mut self) {
        if let Some(project) = self.selected_project() {
            self.project_detail = Some(project.clone());
            self.view = View::ProjectDetail;
            // Ensure we're connected to projects WebSocket
            if self.projects_ws.is_none() {
                self.connect_projects_ws();
            }
        }
    }

    pub async fn enter_task_view(&mut self) -> anyhow::Result<()> {
        if let Some(project) = self.selected_project() {
            let project_id = project.id;
            self.view = View::Tasks;
            self.task_detail = None;
            self.load_tasks(project_id).await?;
            // Connect to tasks WebSocket stream for real-time updates
            self.connect_tasks_ws(project_id);
        }
        Ok(())
    }

    pub fn enter_task_detail_view(&mut self) {
        if let Some(task) = self.selected_task() {
            self.task_detail = Some(task.clone());
            self.view = View::TaskDetail;
        }
    }

    pub fn start_input(&mut self, field: InputField) {
        self.input_mode = InputMode::Editing;
        self.input_field = field;
        self.input.clear();
        self.input_cursor_row = 0;
        self.input_cursor_col = 0;
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_field = InputField::None;
        self.input.clear();
    }

    /// Get the current line of text at the cursor position
    fn get_current_line(&self) -> &str {
        let line_idx = self.input_cursor_row.min(
            self.input.lines().count().saturating_sub(1)
        );
        self.input.lines().nth(line_idx).unwrap_or("")
    }

    /// Get the total number of lines in the input
    fn input_line_count(&self) -> usize {
        if self.input.is_empty() {
            1
        } else {
            self.input.lines().count()
        }
    }

    /// Move cursor left by one character
    pub fn move_cursor_left(&mut self) {
        if self.input_cursor_col > 0 {
            self.input_cursor_col -= 1;
        } else if self.input_cursor_row > 0 {
            // Move to end of previous line
            self.input_cursor_row -= 1;
            self.input_cursor_col = self.get_current_line().len();
        }
    }

    /// Move cursor right by one character
    pub fn move_cursor_right(&mut self) {
        let current_line = self.get_current_line();
        if self.input_cursor_col < current_line.len() {
            self.input_cursor_col += 1;
        } else if self.input_cursor_row < self.input_line_count().saturating_sub(1) {
            // Move to start of next line
            self.input_cursor_row += 1;
            self.input_cursor_col = 0;
        }
    }

    /// Move cursor up by one line
    pub fn move_cursor_up(&mut self) {
        if self.input_cursor_row > 0 {
            self.input_cursor_row -= 1;
            // Clamp column to current line length
            let line_len = self.get_current_line().len();
            if self.input_cursor_col > line_len {
                self.input_cursor_col = line_len;
            }
        }
    }

    /// Move cursor down by one line
    pub fn move_cursor_down(&mut self) {
        let line_count = self.input_line_count();
        if self.input_cursor_row < line_count.saturating_sub(1) {
            self.input_cursor_row += 1;
            // Clamp column to current line length
            let line_len = self.get_current_line().len();
            if self.input_cursor_col > line_len {
                self.input_cursor_col = line_len;
            }
        }
    }

    /// Move cursor to the start of the current line
    pub fn move_cursor_home(&mut self) {
        self.input_cursor_col = 0;
    }

    /// Move cursor to the end of the current line
    pub fn move_cursor_end(&mut self) {
        self.input_cursor_col = self.get_current_line().len();
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        let lines: Vec<&str> = if self.input.is_empty() {
            vec![""]
        } else {
            self.input.lines().collect()
        };

        let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let row = self.input_cursor_row.min(new_lines.len().saturating_sub(1));

        // Ensure we have enough lines
        while new_lines.len() <= row {
            new_lines.push(String::new());
        }

        let line = &mut new_lines[row];
        let col = self.input_cursor_col.min(line.len());

        line.insert(col, c);
        self.input_cursor_col += 1;

        self.input = new_lines.join("\n");
    }

    /// Insert a newline at the cursor position
    pub fn insert_newline(&mut self) {
        let lines: Vec<&str> = if self.input.is_empty() {
            vec![""]
        } else {
            self.input.lines().collect()
        };

        let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let row = self.input_cursor_row.min(new_lines.len().saturating_sub(1));

        // Ensure we have enough lines
        while new_lines.len() <= row {
            new_lines.push(String::new());
        }

        let line = &mut new_lines[row];
        let col = self.input_cursor_col.min(line.len());

        // Split the line at the cursor
        let after_cursor = line[col..].to_string();
        line.truncate(col);

        // Insert new line after current
        new_lines.insert(row + 1, after_cursor);

        self.input_cursor_row += 1;
        self.input_cursor_col = 0;

        self.input = new_lines.join("\n");
    }

    /// Delete character before cursor (Backspace)
    pub fn delete_backward(&mut self) {
        if self.input_cursor_col > 0 {
            // Delete character in current line
            let lines: Vec<&str> = if self.input.is_empty() {
                return;
            } else {
                self.input.lines().collect()
            };

            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.input_cursor_row.min(new_lines.len().saturating_sub(1));

            let line = &mut new_lines[row];
            let col = self.input_cursor_col;

            if !line.is_empty() && col > 0 {
                line.remove(col - 1);
                self.input_cursor_col -= 1;
                self.input = new_lines.join("\n");
            }
        } else if self.input_cursor_row > 0 {
            // Delete newline and merge with previous line
            let lines: Vec<&str> = self.input.lines().collect();
            if lines.len() <= 1 {
                return;
            }

            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.input_cursor_row;
            let current_line_len = new_lines[row].len();

            // Remove the line to be merged
            let next_line = new_lines.remove(row);

            // Now borrow prev_line
            let prev_line = &mut new_lines[row - 1];
            prev_line.push_str(&next_line);

            self.input_cursor_row -= 1;
            self.input_cursor_col = prev_line.len() - current_line_len;

            self.input = new_lines.join("\n");
        }
    }

    /// Delete character at cursor (Delete key)
    pub fn delete_forward(&mut self) {
        let current_line = self.get_current_line();
        if self.input_cursor_col < current_line.len() {
            // Delete character in current line
            let lines: Vec<&str> = if self.input.is_empty() {
                return;
            } else {
                self.input.lines().collect()
            };

            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.input_cursor_row.min(new_lines.len().saturating_sub(1));

            let line = &mut new_lines[row];
            let col = self.input_cursor_col;

            if !line.is_empty() && col < line.len() {
                line.remove(col);
                self.input = new_lines.join("\n");
            }
        } else if self.input_cursor_row < self.input_line_count().saturating_sub(1) {
            // Delete newline and merge with next line
            let lines: Vec<&str> = self.input.lines().collect();
            if lines.len() <= 1 {
                return;
            }

            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.input_cursor_row;

            // Remove next line first
            let next_line = new_lines.remove(row + 1);

            // Now borrow current line
            let current_line = &mut new_lines[row];
            current_line.push_str(&next_line);

            self.input = new_lines.join("\n");
        }
    }

    pub async fn submit_input(&mut self) -> anyhow::Result<()> {
        // For descriptions, allow multi-line input
        // For names/titles, trim whitespace
        let input = match self.input_field {
            InputField::ProjectDescription | InputField::TaskDescription => {
                // Remove leading/trailing empty lines but preserve internal newlines
                let lines: Vec<&str> = self.input.lines().collect();
                let start = lines.iter().position(|s| !s.trim().is_empty()).unwrap_or(lines.len());
                let end = lines.iter().rposition(|s| !s.trim().is_empty()).unwrap_or(0);
                if start > end {
                    String::new()
                } else {
                    lines[start..=end].join("\n")
                }
            }
            _ => self.input.trim().to_string(),
        };

        if input.is_empty() {
            self.cancel_input();
            return Ok(());
        }

        match self.input_field {
            InputField::ProjectName => {
                if self.view == View::Projects {
                    self.create_project(input).await?;
                } else if self.view == View::ProjectDetail {
                    self.update_project_detail(Some(input), None, None).await?;
                    self.set_status("Project name updated");
                }
            }
            InputField::ProjectDescription => {
                self.update_project_detail(None, Some(input), None).await?;
                self.set_status("Project description updated");
            }
            InputField::ProjectRepoPath => {
                self.update_project_detail(None, None, Some(input)).await?;
                self.set_status("Project repository path updated");
            }
            InputField::TaskTitle => {
                if self.view == View::Tasks {
                    self.create_task(input).await?;
                } else if self.view == View::TaskDetail {
                    self.update_task_detail(Some(input), None).await?;
                    self.set_status("Task title updated");
                }
            }
            InputField::TaskDescription => {
                self.update_task_detail(None, Some(input)).await?;
                self.set_status("Task description updated");
            }
            InputField::None => {}
        }

        self.cancel_input();
        Ok(())
    }
}
