use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, config::Config, models::Project};

/// Projects list component
#[derive(Default)]
pub struct Projects {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    projects: Vec<Project>,
    selected_index: usize,
}

impl Projects {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_projects(&mut self, projects: Vec<Project>) {
        self.projects = projects;
        if self.selected_index >= self.projects.len() {
            self.selected_index = self.projects.len().saturating_sub(1);
        }
    }

    pub fn selected_project(&self) -> Option<&Project> {
        self.projects.get(self.selected_index)
    }

    pub fn next(&mut self) {
        if !self.projects.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.projects.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.projects.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.projects.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }
}

impl Component for Projects {
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
            Action::NextProject => self.next(),
            Action::PreviousProject => self.previous(),
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let items: Vec<ListItem> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, project)| {
                let description = project
                    .description
                    .as_ref()
                    .map(|d| format!(" - {}", d))
                    .unwrap_or_default();

                let content = Line::from(vec![
                    Span::styled(
                        &project.name,
                        if i == self.selected_index {
                            Style::new().yellow().bold()
                        } else {
                            Style::new().white()
                        },
                    ),
                    Span::styled(description, Style::new().gray()),
                ]);

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Projects ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            )
            .highlight_style(Style::new().reversed())
            .highlight_symbol("> ");

        frame.render_widget(list, area);
        Ok(())
    }
}
