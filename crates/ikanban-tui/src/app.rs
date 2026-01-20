use uuid::Uuid;

use crate::api::ApiClient;
use crate::models::{CreateProject, CreateTask, Project, Task, TaskStatus, UpdateTask};

/// Current view/screen in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Projects,
    ProjectDetail,
    Tasks,
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

    // Status message
    pub status_message: Option<String>,

    // Running flag
    pub running: bool,
}

/// Which field is being edited
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    None,
    ProjectName,
    ProjectDescription,
    TaskTitle,
}

impl App {
    pub fn new(server_url: &str) -> Self {
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
            status_message: None,
            running: true,
        }
    }

    pub fn set_status(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
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
        };
        self.selected_task_index = 0;
    }

    pub fn previous_column(&mut self) {
        self.selected_column = match self.selected_column {
            TaskStatus::Todo => TaskStatus::Done,
            TaskStatus::InProgress => TaskStatus::Todo,
            TaskStatus::Done => TaskStatus::InProgress,
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
        self.load_projects().await?;
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
            };
            self.api.create_task(&payload).await?;
            self.load_tasks(project.id).await?;
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
            };
            self.api.update_task(id, &payload).await?;
            self.load_tasks(project_id).await?;
        }
        Ok(())
    }

    pub fn enter_project_view(&mut self) {
        self.view = View::Projects;
        self.selected_task_index = 0;
        self.selected_column = TaskStatus::Todo;
        self.project_detail = None;
    }

    pub fn enter_project_detail_view(&mut self) {
        if let Some(project) = self.selected_project() {
            self.project_detail = Some(project.clone());
            self.view = View::ProjectDetail;
        }
    }

    pub async fn enter_task_view(&mut self) -> anyhow::Result<()> {
        if let Some(project) = self.selected_project() {
            let project_id = project.id;
            self.view = View::Tasks;
            self.load_tasks(project_id).await?;
        }
        Ok(())
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
            InputField::TaskTitle => {
                self.create_task(input).await?;
            }
            InputField::None => {}
        }

        self.cancel_input();
        Ok(())
    }
}
