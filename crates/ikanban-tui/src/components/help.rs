use ratatui::{
    layout::{Rect, Size},
    prelude::*,
    widgets::*,
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, config::Config};

/// Help modal component
#[derive(Default)]
pub struct Help {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    visible: bool,
    selected_index: usize,
    shortcuts: Vec<(String, String)>,
    title: String,
}

impl Help {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_shortcuts(&mut self, shortcuts: Vec<(String, String)>, title: String) {
        self.shortcuts = shortcuts;
        self.title = title;
        self.selected_index = 0;
    }

    pub fn next_item(&mut self) {
        let max_index = self.shortcuts.len().saturating_sub(1);
        if max_index > 0 {
            self.selected_index = (self.selected_index + 1).min(max_index);
        }
    }

    pub fn previous_item(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
}

impl Component for Help {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn init(&mut self, area: Size) -> color_eyre::Result<()> {
        // Initialize with empty shortcuts
        self.shortcuts = vec![
            ("j/k".to_string(), "Navigate".to_string()),
            ("Enter".to_string(), "Select".to_string()),
            ("q/Esc".to_string(), "Close".to_string()),
        ];
        self.title = "Help".to_string();
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::NextTask | Action::NextProject => self.next_item(),
            Action::PreviousTask | Action::PreviousProject => self.previous_item(),
            Action::CloseHelpModal | Action::Quit | Action::CancelInput => self.close(),
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        if !self.visible {
            return Ok(());
        }

        // Calculate modal size based on content
        let max_key_width = self
            .shortcuts
            .iter()
            .map(|(key, _)| key.len())
            .max()
            .unwrap_or(0);
        let max_desc_width = self
            .shortcuts
            .iter()
            .map(|(_, desc)| desc.len())
            .max()
            .unwrap_or(0);

        let content_width = (max_key_width + max_desc_width + 7) as u16;
        let content_height = (self.shortcuts.len() + 4) as u16;

        let term_size = frame.size();
        let modal_width = content_width.min(term_size.width - 4).max(40);
        let modal_height = content_height.min(term_size.height - 4).max(10);

        let x = (term_size.width - modal_width) / 2;
        let y = (term_size.height - modal_height) / 2;

        let modal_area = Rect {
            x,
            y,
            width: modal_width,
            height: modal_height,
        };

        // Clear background
        frame.render_widget(Clear, modal_area);

        let items: Vec<ListItem> = self
            .shortcuts
            .iter()
            .enumerate()
            .map(|(i, (key, desc))| {
                let is_selected = i == self.selected_index;
                let key_style = if is_selected {
                    Style::new().yellow().bold()
                } else {
                    Style::new().cyan()
                };
                let desc_style = if is_selected {
                    Style::new().white()
                } else {
                    Style::new().gray()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:>width$} | ", key, width = max_key_width),
                        key_style,
                    ),
                    Span::styled(desc.clone(), desc_style),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(" {} ", self.title))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .highlight_style(Style::new().reversed());

        frame.render_widget(list, modal_area);

        // Footer
        let footer_area = Rect {
            x: modal_area.x + 1,
            y: modal_area.y + modal_area.height - 2,
            width: modal_area.width - 2,
            height: 1,
        };
        let footer = Paragraph::new(" j/k: Navigate, Enter/Esc: Close ")
            .style(Style::new().dark_gray())
            .block(Block::default());
        frame.render_widget(footer, footer_area);

        Ok(())
    }
}
