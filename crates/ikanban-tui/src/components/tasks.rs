use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, config::Config, models::Task, models::TaskStatus};

/// Tasks view component with columns
#[derive(Default)]
pub struct Tasks {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    tasks: Vec<Task>,
    selected_task_index: usize,
    selected_column: TaskStatus,
}

impl Tasks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tasks(&mut self, tasks: Vec<Task>) {
        self.tasks = tasks;
        self.selected_task_index = 0;
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn tasks_in_column(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == status).collect()
    }

    pub fn selected_task(&self) -> Option<&Task> {
        let tasks_in_column = self.tasks_in_column(self.selected_column);
        tasks_in_column.get(self.selected_task_index).copied()
    }

    pub fn selected_column(&self) -> TaskStatus {
        self.selected_column
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
            TaskStatus::InProgress => TaskStatus::InReview,
            TaskStatus::InReview => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Todo,
            TaskStatus::Cancelled => TaskStatus::Todo,
        };
        self.selected_task_index = 0;
    }

    pub fn previous_column(&mut self) {
        self.selected_column = match self.selected_column {
            TaskStatus::Todo => TaskStatus::Done,
            TaskStatus::InProgress => TaskStatus::Todo,
            TaskStatus::InReview => TaskStatus::InProgress,
            TaskStatus::Done => TaskStatus::InReview,
            TaskStatus::Cancelled => TaskStatus::Todo,
        };
        self.selected_task_index = 0;
    }
}

impl Component for Tasks {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::NextTask => self.next_task(),
            Action::PreviousTask => self.previous_task(),
            Action::NextColumn => self.next_column(),
            Action::PreviousColumn => self.previous_column(),
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area);

        self.draw_column(frame, columns[0], TaskStatus::Todo, "Todo");
        self.draw_column(frame, columns[1], TaskStatus::InProgress, "In Progress");
        self.draw_column(frame, columns[2], TaskStatus::Done, "Done");

        Ok(())
    }
}

impl Tasks {
    fn draw_column(&self, frame: &mut Frame, area: Rect, status: TaskStatus, title: &str) {
        let is_selected_column = self.selected_column == status;
        let tasks = self.tasks_in_column(status);

        let items: Vec<ListItem> = tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let style = if is_selected_column && i == self.selected_task_index {
                    Style::new().yellow().bold()
                } else {
                    Style::new().white()
                };

                ListItem::new(task.title.clone()).style(style)
            })
            .collect();

        let border_color = if is_selected_column {
            Color::Cyan
        } else {
            Color::White
        };

        let list = List::new(items).block(
            Block::default()
                .title(format!(" {} ({}) ", title, tasks.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        frame.render_widget(list, area);
    }
}
