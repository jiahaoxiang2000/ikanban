use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{
    action::{Action, InputField},
    components::{Component, help::Help, input::Input, projects::Projects, tasks::Tasks},
    config::Config,
    models::{Project, SubscribeTarget, Task, WsEvent},
    tui::{Event, Tui},
    ws_client::WsClient,
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

/// Application state - The main orchestrator for the TUI
/// 
/// Responsibilities:
/// - Manages global application state (projects, tasks, current view)
/// - Coordinates between components (Projects, Tasks, Input, Help)
/// - Handles WebSocket communication for real-time updates
/// - Routes actions from components to appropriate handlers
/// - Manages view switching and data loading
pub struct App {
    // ===== Core TUI State =====
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,

    // ===== UI Components =====
    // Each component manages its own rendering and local state
    projects_component: Projects,
    tasks_component: Tasks,
    input_component: Input,
    help_component: Help,

    // ===== Application Data =====
    pub ws_client: WsClient,
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub selected_project_index: usize,
    pub selected_task_index: usize,
    pub selected_column: crate::models::TaskStatus,
    pub status_message: Option<String>,
    pub help_shortcuts: Vec<(String, String)>,
    pub help_title: String,
    pub current_project_id: Option<uuid::Uuid>,
    pub is_editing: bool, // Track if we're editing (true) or creating (false)

    // ===== WebSocket State =====
    ws_event_rx: Option<mpsc::UnboundedReceiver<WsEvent>>,
    current_session_id: Option<uuid::Uuid>,
    executions: Vec<crate::models::ExecutionProcess>,
    current_execution_logs: Vec<crate::models::ExecutionProcessLog>,
    selected_execution_index: usize,
    log_view_line_offset: usize,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64, server_url: &str) -> color_eyre::Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (ws_event_tx, ws_event_rx) = mpsc::unbounded_channel::<WsEvent>();

        let projects_component = Projects::new();
        let tasks_component = Tasks::new();
        let input_component = Input::new();
        let help_component = Help::new();

        let ws_client = WsClient::new(server_url, ws_event_tx);

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
            ws_client,
            projects: Vec::new(),
            tasks: Vec::new(),
            selected_project_index: 0,
            selected_task_index: 0,
            selected_column: crate::models::TaskStatus::Todo,
            status_message: None,
            help_shortcuts: Vec::new(),
            help_title: String::new(),
            current_project_id: None,
            is_editing: false,
            ws_event_rx: Some(ws_event_rx),
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
        self.projects_component
            .register_action_handler(self.action_tx.clone())?;
        self.tasks_component
            .register_action_handler(self.action_tx.clone())?;
        self.input_component
            .register_action_handler(self.action_tx.clone())?;
        self.help_component
            .register_action_handler(self.action_tx.clone())?;

        // Load initial data and subscribe to projects
        if let Err(e) = self.load_projects().await {
            self.set_status(&format!("Failed to connect: {}", e));
        } else {
            if let Err(e) = self.ws_client.subscribe(SubscribeTarget::Projects).await {
                self.set_status(&format!("Failed to subscribe to projects: {}", e));
            } else {
                self.set_status("Connected to projects stream");
            }
        }

        // Initialize help shortcuts
        self.update_help_shortcuts();

        // Register config handlers
        self.projects_component
            .register_config_handler(self.config.clone())?;
        self.tasks_component
            .register_config_handler(self.config.clone())?;
        self.input_component
            .register_config_handler(self.config.clone())?;
        self.help_component
            .register_config_handler(self.config.clone())?;

        // Initialize components
        self.projects_component.init(tui.size()?)?;
        self.tasks_component.init(tui.size()?)?;
        self.input_component.init(tui.size()?)?;
        self.help_component.init(tui.size()?)?;

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.process_ws_events().await?;
            self.handle_actions(&mut tui).await?;
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

    /// Handle terminal events and convert them to actions
    /// Events are broadcast to all components, which may emit actions in response
    async fn handle_events(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        
        // Convert terminal events to actions
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => self.handle_key_event(key)?,
            _ => {}
        }

        // Broadcast event to all components
        // Components may emit actions in response to events
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

    /// Handle keyboard input and convert to actions
    /// Special handling for input modal (intercepts all keys when visible)
    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        let action_tx = self.action_tx.clone();

        // ===== Input Modal Key Handling =====
        // When input modal is visible, intercept all keys for text editing
        if self.input_component.is_visible() {
            use crossterm::event::KeyCode;
            match key.code {
                KeyCode::Esc => {
                    action_tx.send(Action::CancelInput)?;
                    return Ok(());
                }
                KeyCode::Enter if key.modifiers.is_empty() => {
                    action_tx.send(Action::SubmitInput)?;
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    action_tx.send(Action::InsertChar(c))?;
                    return Ok(());
                }
                KeyCode::Backspace => {
                    action_tx.send(Action::DeleteBackward)?;
                    return Ok(());
                }
                KeyCode::Delete => {
                    action_tx.send(Action::DeleteForward)?;
                    return Ok(());
                }
                KeyCode::Left => {
                    action_tx.send(Action::MoveCursorLeft)?;
                    return Ok(());
                }
                KeyCode::Right => {
                    action_tx.send(Action::MoveCursorRight)?;
                    return Ok(());
                }
                KeyCode::Up => {
                    action_tx.send(Action::MoveCursorUp)?;
                    return Ok(());
                }
                KeyCode::Down => {
                    action_tx.send(Action::MoveCursorDown)?;
                    return Ok(());
                }
                KeyCode::Home => {
                    action_tx.send(Action::MoveCursorHome)?;
                    return Ok(());
                }
                KeyCode::End => {
                    action_tx.send(Action::MoveCursorEnd)?;
                    return Ok(());
                }
                _ => {}
            }
        }

        // ===== Normal Mode Key Handling =====
        // Look up keybinding for current mode
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

    async fn handle_actions(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }

            match action {
                // ===== Terminal Actions =====
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                
                // ===== Modal Actions =====
                Action::Help => self.toggle_help(),
                Action::CloseHelpModal => self.close_help(),

                // ===== View Switching Actions =====
                Action::EnterTasksView => {
                    // Switch from Projects view to Tasks view
                    if self.mode == Mode::Projects {
                        if let Some(project) = self.projects_component.selected_project() {
                            let project_id = project.id;
                            self.mode = Mode::Tasks;
                            self.update_help_shortcuts();
                            
                            // Load tasks for the selected project
                            if let Err(e) = self.load_tasks(project_id).await {
                                self.set_status(&format!("Failed to load tasks: {}", e));
                            } else {
                                self.connect_tasks_ws(project_id).await;
                            }
                        }
                    }
                }
                Action::EnterProjectsView => {
                    // Switch from Tasks view back to Projects view
                    if self.mode == Mode::Tasks {
                        self.mode = Mode::Projects;
                        self.update_help_shortcuts();
                        self.disconnect_tasks_ws().await;
                    }
                }

                // ===== Navigation Actions =====
                Action::NextProject => {
                    if self.mode == Mode::Projects {
                        self.projects_component.next();
                        self.selected_project_index = self.projects_component.selected_index;
                    }
                }
                Action::PreviousProject => {
                    if self.mode == Mode::Projects {
                        self.projects_component.previous();
                        self.selected_project_index = self.projects_component.selected_index;
                    }
                }

                _ => {}
            }
            
            // ===== Component Updates =====
            // Update the active component based on current mode
            match self.mode {
                Mode::Projects => {
                    if let Some(action) = self.projects_component.update(action.clone())? {
                        self.action_tx.send(action)?;
                    }
                }
                Mode::Tasks => {
                    if let Some(action) = self.tasks_component.update(action.clone())? {
                        self.action_tx.send(action)?;
                    }
                }
                _ => {}
            }
            
            // Always update modal components (they overlay any view)
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
            
            // ===== Draw Main View =====
            // Render the component corresponding to the current mode
            match self.mode {
                Mode::Projects => {
                    if let Err(err) = self.projects_component.draw(frame, area) {
                        let _ = self
                            .action_tx
                            .send(Action::Error(format!("Failed to draw projects: {:?}", err)));
                    }
                }
                Mode::Tasks => {
                    if let Err(err) = self.tasks_component.draw(frame, area) {
                        let _ = self
                            .action_tx
                            .send(Action::Error(format!("Failed to draw tasks: {:?}", err)));
                    }
                }
                Mode::ProjectDetail | Mode::TaskDetail | Mode::ExecutionLogs => {
                    // TODO: Implement these views
                }
            }
            
            // ===== Draw Modal Overlays =====
            // Modals are always drawn on top if visible
            if let Err(err) = self.input_component.draw(frame, area) {
                let _ = self
                    .action_tx
                    .send(Action::Error(format!("Failed to draw input: {:?}", err)));
            }
            if let Err(err) = self.help_component.draw(frame, area) {
                let _ = self
                    .action_tx
                    .send(Action::Error(format!("Failed to draw help: {:?}", err)));
            }
        })?;
        Ok(())
    }

    // ===== UI Helper Methods =====
    
    /// Set status message (displayed in status bar)
    pub fn set_status(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
    }

    /// Toggle help modal visibility
    pub fn toggle_help(&mut self) {
        self.help_component.toggle();
        self.help_component
            .set_shortcuts(self.help_shortcuts.clone(), self.help_title.clone());
    }

    /// Close help modal
    pub fn close_help(&mut self) {
        self.help_component.close();
    }

    /// Update help shortcuts based on current mode
    fn update_help_shortcuts(&mut self) {
        self.help_shortcuts = match self.mode {
            Mode::Projects => vec![
                ("j/k".to_string(), "Navigate projects".to_string()),
                ("Enter".to_string(), "Open project tasks".to_string()),
                ("n".to_string(), "New project".to_string()),
                ("e".to_string(), "Edit project".to_string()),
                ("d".to_string(), "Delete project".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("q".to_string(), "Quit".to_string()),
            ],
            Mode::Tasks => vec![
                ("h/l".to_string(), "Switch column".to_string()),
                ("j/k".to_string(), "Navigate tasks".to_string()),
                ("Space".to_string(), "Move task to next status".to_string()),
                ("n".to_string(), "New task".to_string()),
                ("e".to_string(), "Edit task".to_string()),
                ("d".to_string(), "Delete task".to_string()),
                ("Enter".to_string(), "Task details".to_string()),
                ("Esc".to_string(), "Back to projects".to_string()),
                ("?".to_string(), "Show help".to_string()),
                ("q".to_string(), "Quit".to_string()),
            ],
            _ => vec![
                ("j/k".to_string(), "Navigate".to_string()),
                ("Esc".to_string(), "Back".to_string()),
                ("?".to_string(), "Help".to_string()),
            ],
        };
        self.help_title = match self.mode {
            Mode::Projects => "Projects View",
            Mode::Tasks => "Tasks View",
            Mode::ProjectDetail => "Project Detail",
            Mode::TaskDetail => "Task Detail",
            Mode::ExecutionLogs => "Execution Logs",
        }
        .to_string();
    }

    // ===== Data Loading Methods =====
    
    /// Load projects from API and update Projects component
    pub async fn load_projects(&mut self) -> anyhow::Result<()> {
        self.projects = self.ws_client.list_projects().await?;
        if self.selected_project_index >= self.projects.len() {
            self.selected_project_index = self.projects.len().saturating_sub(1);
        }
        // Update projects component
        self.projects_component.set_projects(self.projects.clone());
        Ok(())
    }

    /// Load tasks for a specific project and update Tasks component
    pub async fn load_tasks(&mut self, project_id: uuid::Uuid) -> anyhow::Result<()> {
        self.tasks = self.ws_client.list_tasks(project_id).await?;
        self.selected_task_index = 0;
        // Update tasks component
        self.tasks_component.set_tasks(self.tasks.clone());
        Ok(())
    }

    // ===== WebSocket Management Methods =====
    
    /// Subscribe to task updates for a specific project
    /// Automatically unsubscribes from previous project if any
    pub async fn connect_tasks_ws(&mut self, project_id: uuid::Uuid) {
        // Unsubscribe from previous project's tasks if any
        if let Some(old_project_id) = self.current_project_id {
            let _ = self
                .ws_client
                .unsubscribe(SubscribeTarget::Tasks {
                    project_id: old_project_id,
                })
                .await;
        }

        // Subscribe to new project's tasks
        if let Err(e) = self
            .ws_client
            .subscribe(SubscribeTarget::Tasks { project_id })
            .await
        {
            self.set_status(&format!("Failed to subscribe to tasks: {}", e));
        } else {
            self.current_project_id = Some(project_id);
            self.set_status("Connected to tasks stream");
        }
    }

    /// Unsubscribe from task updates for current project
    pub async fn disconnect_tasks_ws(&mut self) {
        if let Some(project_id) = self.current_project_id {
            let _ = self
                .ws_client
                .unsubscribe(SubscribeTarget::Tasks { project_id })
                .await;
        }
        self.current_project_id = None;
    }

    /// Process all pending WebSocket events
    /// Called in main loop to handle real-time updates
    pub async fn process_ws_events(&mut self) -> color_eyre::Result<()> {
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

    /// Handle a single WebSocket event
    /// Updates local state and pushes changes to components
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
                    self.tasks.sort_by(|a, b| a.status.cmp(&b.status));
                    self.tasks_component.set_tasks(self.tasks.clone());
                    self.set_status("New task created");
                }
            }
            WsEvent::TaskUpdated(task) => {
                if Some(task.project_id) == self.current_project_id {
                    if let Some(existing) = self.tasks.iter_mut().find(|t| t.id == task.id) {
                        *existing = task.clone();
                    }
                    self.tasks.sort_by(|a, b| a.status.cmp(&b.status));
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
            _ => {}
        }
    }
}
