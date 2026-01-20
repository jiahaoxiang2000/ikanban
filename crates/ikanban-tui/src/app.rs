use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{
    action::{Action, InputField},
    api::ApiClient,
    components::{Component, help::Help, input::Input, projects::Projects, tasks::Tasks},
    config::Config,
    models::{Project, Task, WsEvent},
    tui::{Event, Tui},
    ws::WebSocketClient,
};

/// Current view/screen in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Projects,
    Tasks,
    ProjectDetail,
    TaskDetail,
    ExecutionLogs,
}

/// Application state
pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,

    // Component references for direct access
    projects_component: Projects,
    tasks_component: Tasks,
    input_component: Input,
    help_component: Help,

    // iKanban specific state
    pub api: ApiClient,
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub selected_project_index: usize,
    pub selected_task_index: usize,
    pub selected_column: crate::models::TaskStatus,
    pub status_message: Option<String>,
    pub view: Mode,
    pub help_shortcuts: Vec<(String, String)>,
    pub help_title: String,
    pub current_project_id: Option<uuid::Uuid>,

    // WebSocket state
    ws_event_rx: Option<mpsc::UnboundedReceiver<WsEvent>>,
    ws_event_tx: Option<mpsc::UnboundedSender<WsEvent>>,
    projects_ws: Option<WebSocketClient>,
    tasks_ws: Option<WebSocketClient>,
    execution_logs_ws: Option<WebSocketClient>,
    current_session_id: Option<uuid::Uuid>,
    executions: Vec<crate::models::ExecutionProcess>,
    current_execution_logs: Vec<crate::models::ExecutionProcessLog>,
    selected_execution_index: usize,
    log_view_line_offset: usize,
}

impl App {
    pub fn new(
        tick_rate: f64,
        frame_rate: f64,
        server_url: &str,
    ) -> color_eyre::Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (ws_event_tx, ws_event_rx) = mpsc::unbounded_channel::<WsEvent>();

        let projects_component = Projects::new();
        let tasks_component = Tasks::new();
        let input_component = Input::new();
        let help_component = Help::new();

        Ok(Self {
            config: Config::new()?,
            tick_rate,
            frame_rate,
            should_quit: false,
            should_suspend: false,
            mode: Mode::Projects,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            projects_component,
            tasks_component,
            input_component,
            help_component,
            api: ApiClient::new(server_url),
            projects: Vec::new(),
            tasks: Vec::new(),
            selected_project_index: 0,
            selected_task_index: 0,
            selected_column: crate::models::TaskStatus::Todo,
            status_message: None,
            view: Mode::Projects,
            help_shortcuts: Vec::new(),
            help_title: String::new(),
            current_project_id: None,
            ws_event_rx: Some(ws_event_rx),
            ws_event_tx: Some(ws_event_tx),
            projects_ws: None,
            tasks_ws: None,
            execution_logs_ws: None,
            current_session_id: None,
            executions: Vec::new(),
            current_execution_logs: Vec::new(),
            selected_execution_index: 0,
            log_view_line_offset: 0,
        })
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        // Register action handlers
        self.projects_component.register_action_handler(self.action_tx.clone())?;
        self.tasks_component.register_action_handler(self.action_tx.clone())?;
        self.input_component.register_action_handler(self.action_tx.clone())?;
        self.help_component.register_action_handler(self.action_tx.clone())?;

        // Load initial data
        if let Err(e) = self.load_projects().await {
            self.set_status(&format!("Failed to connect: {}", e));
        } else {
            self.connect_projects_ws();
        }

        // Initialize help shortcuts
        self.update_help_shortcuts();

        // Register config handlers
        self.projects_component.register_config_handler(self.config.clone())?;
        self.tasks_component.register_config_handler(self.config.clone())?;
        self.input_component.register_config_handler(self.config.clone())?;
        self.help_component.register_config_handler(self.config.clone())?;

        // Initialize components
        self.projects_component.init(tui.size()?)?;
        self.tasks_component.init(tui.size()?)?;
        self.input_component.init(tui.size()?)?;
        self.help_component.init(tui.size()?)?;

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui)?;
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => self.handle_key_event(key)?,
            _ => {}
        }

        // Handle events for all components
        if let Some(action) = self.projects_component.handle_events(Some(event.clone()))? {
            action_tx.send(action)?;
        }
        if let Some(action) = self.tasks_component.handle_events(Some(event.clone()))? {
            action_tx.send(action)?;
        }
        if let Some(action) = self.input_component.handle_events(Some(event.clone()))? {
            action_tx.send(action)?;
        }
        if let Some(action) = self.help_component.handle_events(Some(event.clone()))? {
            action_tx.send(action)?;
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        let action_tx = self.action_tx.clone();
        let Some(keymap) = self.config.keybindings.0.get(&self.mode) else {
            return Ok(());
        };
        match keymap.get(&vec![key]) {
            Some(action) => {
                info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
            }
            _ => {
                self.last_tick_key_events.push(key);
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                }
            }
        }
        Ok(())
    }

    fn handle_actions(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }
            match action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                Action::Help => self.toggle_help(),
                Action::CloseHelpModal => self.close_help(),
                _ => {}
            }
            // Update all components
            if let Some(action) = self.projects_component.update(action.clone())? {
                self.action_tx.send(action)?;
            }
            if let Some(action) = self.tasks_component.update(action.clone())? {
                self.action_tx.send(action)?;
            }
            if let Some(action) = self.input_component.update(action.clone())? {
                self.action_tx.send(action)?;
            }
            if let Some(action) = self.help_component.update(action.clone())? {
                self.action_tx.send(action)?;
            }
        }
        Ok(())
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> color_eyre::Result<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.render(tui)?;
        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        tui.draw(|frame| {
            let area = frame.area();
            // Draw components in order
            if let Err(err) = self.projects_component.draw(frame, area) {
                let _ = self.action_tx.send(Action::Error(format!("Failed to draw projects: {:?}", err)));
            }
            if let Err(err) = self.tasks_component.draw(frame, area) {
                let _ = self.action_tx.send(Action::Error(format!("Failed to draw tasks: {:?}", err)));
            }
            if let Err(err) = self.input_component.draw(frame, area) {
                let _ = self.action_tx.send(Action::Error(format!("Failed to draw input: {:?}", err)));
            }
            if let Err(err) = self.help_component.draw(frame, area) {
                let _ = self.action_tx.send(Action::Error(format!("Failed to draw help: {:?}", err)));
            }
        })?;
        Ok(())
    }

    // iKanban specific methods
    pub fn set_status(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
    }

    pub fn toggle_help(&mut self) {
        self.mode = Mode::Projects;
        self.update_help_shortcuts();
    }

    pub fn close_help(&mut self) {
        self.mode = Mode::Projects;
    }

    fn update_help_shortcuts(&mut self) {
        self.help_shortcuts = match self.view {
            Mode::Projects => vec![
                ("n".to_string(), "New project".to_string()),
                ("j/k".to_string(), "Navigate".to_string()),
                ("Enter".to_string(), "Open tasks".to_string()),
                ("?".to_string(), "Help".to_string()),
                ("q".to_string(), "Quit".to_string()),
            ],
            Mode::Tasks => vec![
                ("n".to_string(), "New task".to_string()),
                ("h/l".to_string(), "Switch column".to_string()),
                ("j/k".to_string(), "Navigate".to_string()),
                ("Space".to_string(), "Move status".to_string()),
                ("Enter".to_string(), "Details".to_string()),
                ("Esc".to_string(), "Back".to_string()),
                ("?".to_string(), "Help".to_string()),
            ],
            _ => vec![
                ("j/k".to_string(), "Navigate".to_string()),
                ("Esc".to_string(), "Back".to_string()),
                ("?".to_string(), "Help".to_string()),
            ],
        };
        self.help_title = match self.view {
            Mode::Projects => "Projects View",
            Mode::Tasks => "Tasks View",
            Mode::ProjectDetail => "Project Detail",
            Mode::TaskDetail => "Task Detail",
            Mode::ExecutionLogs => "Execution Logs",
        }
        .to_string();
    }

    pub async fn load_projects(&mut self) -> anyhow::Result<()> {
        self.projects = self.api.list_projects().await?;
        if self.selected_project_index >= self.projects.len() {
            self.selected_project_index = self.projects.len().saturating_sub(1);
        }
        // Update projects component
        self.projects_component.set_projects(self.projects.clone());
        Ok(())
    }

    pub async fn load_tasks(&mut self, project_id: uuid::Uuid) -> anyhow::Result<()> {
        self.tasks = self.api.list_tasks(project_id).await?;
        self.selected_task_index = 0;
        // Update tasks component
        self.tasks_component.set_tasks(self.tasks.clone());
        Ok(())
    }

    pub fn connect_projects_ws(&mut self) {
        let event_tx = self
            .ws_event_tx()
            .expect("WebSocket event channel should be available");
        self.projects_ws = Some(WebSocketClient::projects(&self.api.base_url(), event_tx));
        self.set_status("Connected to projects stream");
    }

    pub fn connect_tasks_ws(&mut self, project_id: uuid::Uuid) {
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
        self.set_status(&format!("Connected to tasks stream"));
    }

    pub fn disconnect_tasks_ws(&mut self) {
        self.tasks_ws = None;
        self.current_project_id = None;
    }

    fn ws_event_tx(&self) -> Option<mpsc::UnboundedSender<WsEvent>> {
        self.ws_event_tx.clone()
    }

    pub async fn process_ws_events(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut rx) = self.ws_event_rx {
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            for event in events {
                self.handle_ws_event(event).await;
            }
        }
        Ok(())
    }

    pub async fn handle_ws_event(&mut self, event: WsEvent) {
        match event {
            WsEvent::ProjectCreated(project) => {
                self.projects.push(project);
                self.set_status("New project created");
                self.projects_component.set_projects(self.projects.clone());
            }
            WsEvent::ProjectUpdated(project) => {
                if let Some(existing) = self.projects.iter_mut().find(|p| p.id == project.id) {
                    *existing = project.clone();
                }
                self.projects_component.set_projects(self.projects.clone());
                self.set_status("Project updated");
            }
            WsEvent::ProjectDeleted { id } => {
                self.projects.retain(|p| p.id != id);
                if self.selected_project_index >= self.projects.len() {
                    self.selected_project_index = self.projects.len().saturating_sub(1);
                }
                self.projects_component.set_projects(self.projects.clone());
                self.set_status("Project deleted");
            }
            WsEvent::TaskCreated(task) => {
                if Some(task.project_id) == self.current_project_id {
                    self.tasks.push(task.clone());
                    self.tasks
                        .sort_by(|a, b| a.status.cmp(&b.status));
                    self.tasks_component.set_tasks(self.tasks.clone());
                    self.set_status("New task created");
                }
            }
            WsEvent::TaskUpdated(task) => {
                if Some(task.project_id) == self.current_project_id {
                    if let Some(existing) = self.tasks.iter_mut().find(|t| t.id == task.id) {
                        *existing = task.clone();
                    }
                    self.tasks
                        .sort_by(|a, b| a.status.cmp(&b.status));
                    self.tasks_component.set_tasks(self.tasks.clone());
                    self.set_status("Task updated");
                }
            }
            WsEvent::TaskDeleted { id } => {
                self.tasks.retain(|t| t.id != id);
                if self.selected_task_index >= self.tasks.len() {
                    self.selected_task_index = self.tasks.len().saturating_sub(1);
                }
                self.tasks_component.set_tasks(self.tasks.clone());
                self.set_status("Task deleted");
            }
            WsEvent::Connected => {
                self.set_status("WebSocket connected");
            }
            _ => {}
        }
    }
}
