use tokio::sync::mpsc;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::models::{
    CreateProject, CreateTask, Project, Task, TaskStatus, TaskStatusExt, UpdateTask, WsEvent,
};
use crate::ws::WebSocketClient;

/// Current view/screen in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Projects,
    ProjectDetail,
    Tasks,
    TaskDetail,
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
        }
    }

    pub fn set_status(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
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
        self.set_status(&format!("Connected to tasks stream for project {}", project_id));
    }

    /// Disconnect from tasks WebSocket stream
    pub fn disconnect_tasks_ws(&mut self) {
        self.tasks_ws = None;
        self.current_project_id = None;
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
            WsEvent::Log { execution_id: _, content } => {
                tracing::info!("Log: {}", content);
            }
            // Session, Execution, Merge events - log them for now
            WsEvent::SessionCreated(_) => tracing::debug!("Session created"),
            WsEvent::SessionUpdated(_) => tracing::debug!("Session updated"),
            WsEvent::ExecutionCreated(_) => tracing::debug!("Execution created"),
            WsEvent::ExecutionUpdated(_) => tracing::debug!("Execution updated"),
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

    pub async fn update_project_detail(&mut self, name: Option<String>, description: Option<String>, repo_path: Option<String>) -> anyhow::Result<()> {
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

    pub async fn update_task_detail(&mut self, title: Option<String>, description: Option<String>) -> anyhow::Result<()> {
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
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_field = InputField::None;
        self.input.clear();
    }

    pub async fn submit_input(&mut self) -> anyhow::Result<()> {
        let input = self.input.trim().to_string();
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
