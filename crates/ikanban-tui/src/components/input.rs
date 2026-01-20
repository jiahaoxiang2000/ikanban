use ratatui::{
    layout::{Rect, Size},
    prelude::*,
    widgets::*,
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, action::InputField, config::Config};

/// Input modal component for text entry
#[derive(Default)]
pub struct Input {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    visible: bool,
    field: InputField,
    input: String,
    cursor_row: usize,
    cursor_col: usize,
    view: String, // Current view for context
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, field: InputField, view: String) {
        self.visible = true;
        self.field = field;
        self.input.clear();
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.view = view;
    }

    pub fn cancel(&mut self) {
        self.visible = false;
        self.field = InputField::None;
        self.input.clear();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn field(&self) -> InputField {
        self.field
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn title(&self) -> String {
        match self.field {
            InputField::ProjectName => "Project Name".to_string(),
            InputField::ProjectDescription => "Project Description".to_string(),
            InputField::ProjectRepoPath => "Repository Path".to_string(),
            InputField::TaskTitle => "Task Title".to_string(),
            InputField::TaskDescription => "Task Description".to_string(),
            InputField::None => "Input".to_string(),
        }
    }

    // Input editing methods
    pub fn insert_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let lines: Vec<&str> = if self.input.is_empty() {
            vec![""]
        } else {
            self.input.lines().collect()
        };
        let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let row = self.cursor_row.min(new_lines.len().saturating_sub(1));
        while new_lines.len() <= row {
            new_lines.push(String::new());
        }
        let line = &mut new_lines[row];
        let col = self.cursor_col.min(line.len());
        line.insert(col, c);
        self.cursor_col += 1;
        self.input = new_lines.join("\n");
    }

    pub fn insert_newline(&mut self) {
        let lines: Vec<&str> = if self.input.is_empty() {
            vec![""]
        } else {
            self.input.lines().collect()
        };
        let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let row = self.cursor_row.min(new_lines.len().saturating_sub(1));
        while new_lines.len() <= row {
            new_lines.push(String::new());
        }
        let line = &mut new_lines[row];
        let col = self.cursor_col.min(line.len());
        let after_cursor = line[col..].to_string();
        line.truncate(col);
        new_lines.insert(row + 1, after_cursor);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.input = new_lines.join("\n");
    }

    pub fn delete_backward(&mut self) {
        if self.cursor_col > 0 {
            let lines: Vec<&str> = if self.input.is_empty() {
                return;
            } else {
                self.input.lines().collect()
            };
            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.cursor_row.min(new_lines.len().saturating_sub(1));
            let line = &mut new_lines[row];
            let col = self.cursor_col;
            if !line.is_empty() && col > 0 {
                line.remove(col - 1);
                self.cursor_col -= 1;
                self.input = new_lines.join("\n");
            }
        } else if self.cursor_row > 0 {
            let lines: Vec<&str> = self.input.lines().collect();
            if lines.len() <= 1 {
                return;
            }
            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.cursor_row;
            let current_line_len = new_lines[row].len();
            let next_line = new_lines.remove(row);
            let prev_line = &mut new_lines[row - 1];
            prev_line.push_str(&next_line);
            self.cursor_row -= 1;
            self.cursor_col = prev_line.len() - current_line_len;
            self.input = new_lines.join("\n");
        }
    }

    pub fn delete_forward(&mut self) {
        let current_line = self.get_current_line();
        if self.cursor_col < current_line.len() {
            let lines: Vec<&str> = if self.input.is_empty() {
                return;
            } else {
                self.input.lines().collect()
            };
            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.cursor_row.min(new_lines.len().saturating_sub(1));
            let line = &mut new_lines[row];
            let col = self.cursor_col;
            if !line.is_empty() && col < line.len() {
                line.remove(col);
                self.input = new_lines.join("\n");
            }
        } else if self.cursor_row < self.input_line_count().saturating_sub(1) {
            let lines: Vec<&str> = self.input.lines().collect();
            if lines.len() <= 1 {
                return;
            }
            let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            let row = self.cursor_row;
            let next_line = new_lines.remove(row + 1);
            let current_line = &mut new_lines[row];
            current_line.push_str(&next_line);
            self.input = new_lines.join("\n");
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.get_current_line().len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        let current_line = self.get_current_line();
        if self.cursor_col < current_line.len() {
            self.cursor_col += 1;
        } else if self.cursor_row < self.input_line_count().saturating_sub(1) {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let line_len = self.get_current_line().len();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        let line_count = self.input_line_count();
        if self.cursor_row < line_count.saturating_sub(1) {
            self.cursor_row += 1;
            let line_len = self.get_current_line().len();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.get_current_line().len();
    }

    fn get_current_line(&self) -> &str {
        let line_idx = self
            .cursor_row
            .min(self.input.lines().count().saturating_sub(1));
        self.input.lines().nth(line_idx).unwrap_or("")
    }

    fn input_line_count(&self) -> usize {
        if self.input.is_empty() {
            1
        } else {
            self.input.lines().count()
        }
    }
}

impl Component for Input {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn init(&mut self, _area: Size) -> color_eyre::Result<()> {
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::StartInput(field) => {
                self.start(field, "Current".to_string());
            }
            Action::CancelInput => self.cancel(),
            Action::InsertChar(c) => self.insert_char(c),
            Action::InsertNewline => self.insert_newline(),
            Action::DeleteBackward => self.delete_backward(),
            Action::DeleteForward => self.delete_forward(),
            Action::MoveCursorLeft => self.move_cursor_left(),
            Action::MoveCursorRight => self.move_cursor_right(),
            Action::MoveCursorUp => self.move_cursor_up(),
            Action::MoveCursorDown => self.move_cursor_down(),
            Action::MoveCursorHome => self.move_cursor_home(),
            Action::MoveCursorEnd => self.move_cursor_end(),
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        if !self.visible {
            return Ok(());
        }

        let lines: Vec<&str> = if self.input.is_empty() {
            vec![""]
        } else {
            self.input.lines().collect()
        };

        let max_line_width = lines.iter().map(|l| l.len()).max().unwrap_or(10);
        let line_count = lines.len().max(1);

        let popup_width_u16 = (max_line_width + 6).min(80).max(30) as u16;
        let popup_height_u16 = (line_count + 4).min(20).max(6) as u16;

        let term_size = frame.size();
        let percent_x = (popup_width_u16 * 100 / term_size.width).max(1).min(100);
        let percent_y = (popup_height_u16 * 100 / term_size.height).max(1).min(100);

        let popup_area = self.centered_rect(percent_x, percent_y, area);

        frame.render_widget(Clear, popup_area);

        let text = if self.input.is_empty() {
            Text::raw("")
        } else {
            Text::from(self.input.clone())
        };

        let input = Paragraph::new(text).style(Style::new().yellow()).block(
            Block::default()
                .title(format!(" {} ", self.title()))
                .borders(Borders::ALL)
                .border_style(Style::new().cyan()),
        );

        frame.render_widget(input, popup_area);

        // Calculate cursor position
        let cursor_line = self.cursor_row.min(line_count.saturating_sub(1));
        let cursor_col = if cursor_line < lines.len() {
            self.cursor_col.min(lines[cursor_line].len())
        } else {
            self.cursor_col
        };

        let cursor_x = popup_area.x + 1 + cursor_col as u16;
        let cursor_y = popup_area.y + 1 + cursor_line as u16;

        let cursor_x = cursor_x.min(popup_area.x + popup_area.width - 2);
        let cursor_y = cursor_y.min(popup_area.y + popup_area.height - 2);

        frame.set_cursor_position((cursor_x, cursor_y));

        Ok(())
    }
}

impl Input {
    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}
