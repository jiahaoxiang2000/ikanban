use egui::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewLevel {
    Project,
    Task,
    Session,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    MoveSelection(Direction),
    MoveTask(Direction),
    SelectTask,
    CreateTask,
    DeleteTask,
    EditTask,
    StartSession,
    StopSession,
    ToggleMode(Mode),
    JumpToTop,
    JumpToBottom,
    JumpToColumn(usize),
    Search,
    Quit,
    DrillDown,
    GoBack,
}

pub struct KeyboardState {
    pub mode: Mode,
    pub view_level: ViewLevel,
    pub selected_column: usize,
    pub selected_row: usize,
    pub selected_project_index: usize,
    pub selected_session_index: usize,
    pub pending_key: Option<Key>,
    pub command_buffer: String,
    pub last_action: Action,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            view_level: ViewLevel::Project,
            selected_column: 0,
            selected_row: 0,
            selected_project_index: 0,
            selected_session_index: 0,
            pending_key: None,
            command_buffer: String::new(),
            last_action: Action::None,
        }
    }
}

impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key(&mut self, key: Key, modifiers: &egui::Modifiers) -> Action {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key, modifiers),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Visual => self.handle_visual_mode(key),
            Mode::Command => self.handle_command_mode(key),
        }
    }

    fn handle_normal_mode(&mut self, key: Key, modifiers: &egui::Modifiers) -> Action {
        if modifiers.ctrl {
            return self.handle_ctrl_keys(key);
        }

        if let Some(pending) = self.pending_key {
            let action = self.handle_two_key_combo(pending, key);
            self.pending_key = None;
            return action;
        }

        match key {
            Key::H => Action::MoveSelection(Direction::Left),
            Key::J => Action::MoveSelection(Direction::Down),
            Key::K => Action::MoveSelection(Direction::Up),
            Key::L => Action::MoveSelection(Direction::Right),

            Key::ArrowLeft => Action::MoveSelection(Direction::Left),
            Key::ArrowDown => Action::MoveSelection(Direction::Down),
            Key::ArrowUp => Action::MoveSelection(Direction::Up),
            Key::ArrowRight => Action::MoveSelection(Direction::Right),

            Key::G => {
                self.pending_key = Some(Key::G);
                Action::None
            }

            Key::D => {
                self.pending_key = Some(Key::D);
                Action::None
            }

            Key::Num1 => Action::JumpToColumn(0),
            Key::Num2 => Action::JumpToColumn(1),
            Key::Num3 => Action::JumpToColumn(2),
            Key::Num4 => Action::JumpToColumn(3),

            Key::I => Action::ToggleMode(Mode::Insert),
            Key::V => Action::ToggleMode(Mode::Visual),
            Key::Colon => Action::ToggleMode(Mode::Command),

            Key::Enter => Action::DrillDown,
            Key::N => Action::CreateTask,
            Key::E => Action::EditTask,
            Key::S => Action::StartSession,
            Key::X => Action::StopSession,

            Key::Slash => Action::Search,
            Key::Q => Action::Quit,

            _ => Action::None,
        }
    }

    fn handle_two_key_combo(&mut self, first: Key, second: Key) -> Action {
        match (first, second) {
            (Key::G, Key::G) => Action::JumpToTop,
            (Key::D, Key::D) => Action::DeleteTask,
            _ => Action::None,
        }
    }

    fn handle_ctrl_keys(&mut self, key: Key) -> Action {
        match key {
            Key::H => Action::MoveTask(Direction::Left),
            Key::J => Action::MoveTask(Direction::Down),
            Key::K => Action::MoveTask(Direction::Up),
            Key::L => Action::MoveTask(Direction::Right),
            Key::C => Action::Quit,
            _ => Action::None,
        }
    }

    fn handle_insert_mode(&mut self, key: Key) -> Action {
        match key {
            Key::Escape => Action::GoBack,
            _ => Action::None,
        }
    }

    fn handle_visual_mode(&mut self, key: Key) -> Action {
        match key {
            Key::Escape => Action::ToggleMode(Mode::Normal),
            Key::H => Action::MoveSelection(Direction::Left),
            Key::J => Action::MoveSelection(Direction::Down),
            Key::K => Action::MoveSelection(Direction::Up),
            Key::L => Action::MoveSelection(Direction::Right),
            _ => Action::None,
        }
    }

    fn handle_command_mode(&mut self, key: Key) -> Action {
        match key {
            Key::Escape => {
                self.command_buffer.clear();
                Action::ToggleMode(Mode::Normal)
            }
            Key::Enter => {
                let action = self.execute_command();
                self.command_buffer.clear();
                Action::ToggleMode(Mode::Normal);
                action
            }
            _ => Action::None,
        }
    }

    fn execute_command(&self) -> Action {
        match self.command_buffer.trim() {
            "q" | "quit" => Action::Quit,
            "w" | "write" => Action::None,
            "wq" => Action::Quit,
            _ => Action::None,
        }
    }

    pub fn move_selection(
        &mut self,
        direction: Direction,
        max_columns: usize,
        column_sizes: &[usize],
    ) {
        match direction {
            Direction::Left => {
                if self.selected_column > 0 {
                    self.selected_column -= 1;
                    self.selected_row = self
                        .selected_row
                        .min(column_sizes[self.selected_column].saturating_sub(1));
                }
            }
            Direction::Right => {
                if self.selected_column < max_columns.saturating_sub(1) {
                    self.selected_column += 1;
                    self.selected_row = self
                        .selected_row
                        .min(column_sizes[self.selected_column].saturating_sub(1));
                }
            }
            Direction::Up => {
                if self.selected_row > 0 {
                    self.selected_row -= 1;
                }
            }
            Direction::Down => {
                if self.selected_row < column_sizes[self.selected_column].saturating_sub(1) {
                    self.selected_row += 1;
                }
            }
        }
    }

    pub fn jump_to_top(&mut self) {
        self.selected_row = 0;
    }

    pub fn jump_to_bottom(&mut self, column_size: usize) {
        self.selected_row = column_size.saturating_sub(1);
    }

    pub fn jump_to_column(&mut self, column: usize, max_columns: usize, column_sizes: &[usize]) {
        if column < max_columns {
            self.selected_column = column;
            self.selected_row = self
                .selected_row
                .min(column_sizes[column].saturating_sub(1));
        }
    }

    pub fn get_mode_string(&self) -> &str {
        match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::Command => "COMMAND",
        }
    }

    pub fn get_mode_color(&self) -> egui::Color32 {
        match self.mode {
            Mode::Normal => egui::Color32::from_rgb(100, 150, 255),
            Mode::Insert => egui::Color32::from_rgb(100, 255, 100),
            Mode::Visual => egui::Color32::from_rgb(255, 150, 100),
            Mode::Command => egui::Color32::from_rgb(255, 255, 100),
        }
    }

    pub fn get_view_string(&self) -> &str {
        match self.view_level {
            ViewLevel::Project => "PROJECT",
            ViewLevel::Task => "TASK",
            ViewLevel::Session => "SESSION",
        }
    }

    pub fn drill_down(&mut self) -> bool {
        match self.view_level {
            ViewLevel::Project => {
                self.view_level = ViewLevel::Task;
                true
            }
            ViewLevel::Task => {
                self.view_level = ViewLevel::Session;
                true
            }
            ViewLevel::Session => false,
        }
    }

    pub fn go_back(&mut self) -> bool {
        if self.mode != Mode::Normal {
            self.mode = Mode::Normal;
            self.command_buffer.clear();
            return true;
        }
        match self.view_level {
            ViewLevel::Project => false,
            ViewLevel::Task => {
                self.view_level = ViewLevel::Project;
                true
            }
            ViewLevel::Session => {
                self.view_level = ViewLevel::Task;
                true
            }
        }
    }

    pub fn move_project_selection(&mut self, direction: Direction, project_count: usize) {
        if project_count == 0 {
            return;
        }
        match direction {
            Direction::Up | Direction::Left => {
                if self.selected_project_index > 0 {
                    self.selected_project_index -= 1;
                }
            }
            Direction::Down | Direction::Right => {
                if self.selected_project_index < project_count.saturating_sub(1) {
                    self.selected_project_index += 1;
                }
            }
        }
    }

    pub fn move_session_selection(&mut self, direction: Direction, session_count: usize) {
        if session_count == 0 {
            return;
        }
        match direction {
            Direction::Up | Direction::Left => {
                if self.selected_session_index > 0 {
                    self.selected_session_index -= 1;
                }
            }
            Direction::Down | Direction::Right => {
                if self.selected_session_index < session_count.saturating_sub(1) {
                    self.selected_session_index += 1;
                }
            }
        }
    }
}
