#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use directories::ProjectDirs;
use ratatui::style::{Color, Modifier, Style};
use serde::{de::Deserializer, Deserialize};
use tracing::error;

use crate::{action::Action, app::Mode};

pub fn project_name() -> &'static String {
    static PROJECT_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PROJECT_NAME.get_or_init(|| env!("CARGO_CRATE_NAME").to_uppercase().to_string())
}

fn data_folder() -> &'static Option<PathBuf> {
    static DATA_FOLDER: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DATA_FOLDER.get_or_init(|| {
        std::env::var(format!("{}_DATA", project_name()))
            .ok()
            .map(PathBuf::from)
    })
}

fn config_folder() -> &'static Option<PathBuf> {
    static CONFIG_FOLDER: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    CONFIG_FOLDER.get_or_init(|| {
        std::env::var(format!("{}_CONFIG", project_name()))
            .ok()
            .map(PathBuf::from)
    })
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub config_dir: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default, flatten)]
    pub config: AppConfig,
    #[serde(default)]
    pub keybindings: KeyBindings,
    #[serde(default)]
    pub styles: Styles,
}

impl Config {
    pub fn new() -> color_eyre::Result<Self, config::ConfigError> {
        let data_dir = get_data_dir();
        let config_dir = get_config_dir();
        let mut builder = config::Config::builder()
            .set_default("data_dir", data_dir.to_str().unwrap())?
            .set_default("config_dir", config_dir.to_str().unwrap())?;

        // Try to load config files
        let config_files = [
            ("config.json5", config::FileFormat::Json5),
            ("config.json", config::FileFormat::Json),
            ("config.yaml", config::FileFormat::Yaml),
            ("config.toml", config::FileFormat::Toml),
            ("config.ini", config::FileFormat::Ini),
        ];
        let mut found_config = false;
        for (file, format) in &config_files {
            let source = config::File::from(config_dir.join(file))
                .format(*format)
                .required(false);
            builder = builder.add_source(source);
            if config_dir.join(file).exists() {
                found_config = true
            }
        }
        if !found_config {
            error!("No configuration file found. Using default settings.");
        }

        let mut cfg: Self = builder.build()?.try_deserialize()?;

        // Add default keybindings if none are configured
        if cfg.keybindings.0.is_empty() {
            // Common bindings for all modes
            let common_bindings: HashMap<Vec<KeyEvent>, Action> = [
                (
                    parse_key_sequence("q").map_err(|e| config::ConfigError::Message(e))?,
                    Action::Quit,
                ),
                (
                    parse_key_sequence("?").map_err(|e| config::ConfigError::Message(e))?,
                    Action::Help,
                ),
            ]
            .into_iter()
            .collect();

            // Projects mode bindings
            let mut projects_bindings = common_bindings.clone();
            projects_bindings.extend([
                (
                    parse_key_sequence("j").map_err(|e| config::ConfigError::Message(e))?,
                    Action::NextProject,
                ),
                (
                    parse_key_sequence("k").map_err(|e| config::ConfigError::Message(e))?,
                    Action::PreviousProject,
                ),
                (
                    parse_key_sequence("<Enter>").map_err(|e| config::ConfigError::Message(e))?,
                    Action::EnterTasksView,
                ),
                (
                    parse_key_sequence("n").map_err(|e| config::ConfigError::Message(e))?,
                    Action::StartInputForNew(crate::action::InputField::ProjectName),
                ),
                (
                    parse_key_sequence("e").map_err(|e| config::ConfigError::Message(e))?,
                    Action::StartInputForEdit(crate::action::InputField::ProjectName),
                ),
                (
                    parse_key_sequence("d").map_err(|e| config::ConfigError::Message(e))?,
                    Action::DeleteSelectedProject,
                ),
            ]);

            // Tasks mode bindings
            let mut tasks_bindings = common_bindings.clone();
            tasks_bindings.extend([
                (
                    parse_key_sequence("j").map_err(|e| config::ConfigError::Message(e))?,
                    Action::NextTask,
                ),
                (
                    parse_key_sequence("k").map_err(|e| config::ConfigError::Message(e))?,
                    Action::PreviousTask,
                ),
                (
                    parse_key_sequence("h").map_err(|e| config::ConfigError::Message(e))?,
                    Action::PreviousColumn,
                ),
                (
                    parse_key_sequence("l").map_err(|e| config::ConfigError::Message(e))?,
                    Action::NextColumn,
                ),
                (
                    parse_key_sequence("<Esc>").map_err(|e| config::ConfigError::Message(e))?,
                    Action::EnterProjectsView,
                ),
                (
                    parse_key_sequence("n").map_err(|e| config::ConfigError::Message(e))?,
                    Action::StartInputForNew(crate::action::InputField::TaskTitle),
                ),
                (
                    parse_key_sequence("e").map_err(|e| config::ConfigError::Message(e))?,
                    Action::StartInputForEdit(crate::action::InputField::TaskTitle),
                ),
                (
                    parse_key_sequence("d").map_err(|e| config::ConfigError::Message(e))?,
                    Action::DeleteSelectedTask,
                ),
                (
                    parse_key_sequence("<Space>").map_err(|e| config::ConfigError::Message(e))?,
                    Action::MoveTaskToNextStatus,
                ),
            ]);

            cfg.keybindings.0.insert(Mode::Projects, projects_bindings);
            cfg.keybindings.0.insert(Mode::Tasks, tasks_bindings);
            cfg.keybindings
                .0
                .insert(Mode::ProjectDetail, common_bindings.clone());
            cfg.keybindings
                .0
                .insert(Mode::TaskDetail, common_bindings.clone());
            cfg.keybindings
                .0
                .insert(Mode::ExecutionLogs, common_bindings);
        }

        Ok(cfg)
    }
}

pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = data_folder().clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    };
    directory
}

pub fn get_config_dir() -> PathBuf {
    let directory = if let Some(s) = config_folder().clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".config")
    };
    directory
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "ikanban", env!("CARGO_PKG_NAME"))
}

#[derive(Clone, Debug, Default)]
pub struct KeyBindings(pub HashMap<Mode, HashMap<Vec<KeyEvent>, Action>>);

impl<'de> Deserialize<'de> for KeyBindings {
    fn deserialize<D>(deserializer: D) -> color_eyre::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Mode, HashMap<String, Action>>::deserialize(deserializer)?;

        let keybindings = parsed_map
            .into_iter()
            .map(|(mode, inner_map)| {
                let converted_inner_map = inner_map
                    .into_iter()
                    .map(|(key_str, cmd)| (parse_key_sequence(&key_str).unwrap(), cmd))
                    .collect();
                (mode, converted_inner_map)
            })
            .collect();

        Ok(KeyBindings(keybindings))
    }
}

fn parse_key_event(raw: &str) -> color_eyre::Result<KeyEvent, String> {
    let raw_lower = raw.to_ascii_lowercase();
    let (remaining, modifiers) = extract_modifiers(&raw_lower);
    parse_key_code_with_modifiers(remaining, modifiers)
}

fn extract_modifiers(raw: &str) -> (&str, KeyModifiers) {
    let mut modifiers = KeyModifiers::empty();
    let mut current = raw;

    loop {
        match current {
            rest if rest.starts_with("ctrl-") => {
                modifiers.insert(KeyModifiers::CONTROL);
                current = &rest[5..];
            }
            rest if rest.starts_with("alt-") => {
                modifiers.insert(KeyModifiers::ALT);
                current = &rest[4..];
            }
            rest if rest.starts_with("shift-") => {
                modifiers.insert(KeyModifiers::SHIFT);
                current = &rest[6..];
            }
            _ => break,
        };
    }

    (current, modifiers)
}

fn parse_key_code_with_modifiers(
    raw: &str,
    mut modifiers: KeyModifiers,
) -> color_eyre::Result<KeyEvent, String> {
    let c = match raw {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backtab" => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        c if c.len() == 1 => {
            let mut c = c.chars().next().unwrap();
            if modifiers.contains(KeyModifiers::SHIFT) {
                c = c.to_ascii_uppercase();
            }
            KeyCode::Char(c)
        }
        _ => return Err(format!("Unable to parse {raw}")),
    };
    Ok(KeyEvent::new(c, modifiers))
}

pub fn parse_key_sequence(raw: &str) -> color_eyre::Result<Vec<KeyEvent>, String> {
    if raw.chars().filter(|c| *c == '>').count() != raw.chars().filter(|c| *c == '<').count() {
        return Err(format!("Unable to parse `{}`", raw));
    }
    let raw = if !raw.contains("><") {
        let raw = raw.strip_prefix('<').unwrap_or(raw);
        let raw = raw.strip_prefix('>').unwrap_or(raw);
        raw
    } else {
        raw
    };
    let sequences = raw
        .split("><")
        .map(|seq| {
            if let Some(s) = seq.strip_prefix('<') {
                s
            } else if let Some(s) = seq.strip_suffix('>') {
                s
            } else {
                seq
            }
        })
        .collect::<Vec<_>>();

    sequences.into_iter().map(parse_key_event).collect()
}

#[derive(Clone, Debug, Default)]
pub struct Styles(pub HashMap<Mode, HashMap<String, Style>>);

impl<'de> Deserialize<'de> for Styles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Mode, HashMap<String, String>>::deserialize(deserializer)?;

        let styles = parsed_map
            .into_iter()
            .map(|(mode, inner_map)| {
                let converted_inner_map = inner_map
                    .into_iter()
                    .map(|(str, style)| (str, parse_style(&style)))
                    .collect();
                (mode, converted_inner_map)
            })
            .collect();

        Ok(Styles(styles))
    }
}

pub fn parse_style(line: &str) -> Style {
    let (foreground, background) =
        line.split_at(line.to_lowercase().find(" on ").unwrap_or(line.len()));
    let foreground = process_color_string(foreground);
    let background = process_color_string(&background.replace("on ", ""));

    let mut style = Style::default();
    if let Some(fg) = parse_color(&foreground.0) {
        style = style.fg(fg);
    }
    if let Some(bg) = parse_color(&background.0) {
        style = style.bg(bg);
    }
    style = style.add_modifier(foreground.1 | background.1);
    style
}

fn process_color_string(color_str: &str) -> (String, Modifier) {
    let color = color_str
        .replace("grey", "gray")
        .replace("bright ", "")
        .replace("bold ", "")
        .replace("underline ", "")
        .replace("inverse ", "");

    let mut modifiers = Modifier::empty();
    if color_str.contains("underline") {
        modifiers |= Modifier::UNDERLINED;
    }
    if color_str.contains("bold") {
        modifiers |= Modifier::BOLD;
    }
    if color_str.contains("inverse") {
        modifiers |= Modifier::REVERSED;
    }

    (color, modifiers)
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim_start();
    let s = s.trim_end();
    if s.contains("rgb") {
        let red = (s.as_bytes()[3] as char).to_digit(10).unwrap_or_default() as u8;
        let green = (s.as_bytes()[4] as char).to_digit(10).unwrap_or_default() as u8;
        let blue = (s.as_bytes()[5] as char).to_digit(10).unwrap_or_default() as u8;
        let c = 16 + red * 36 + green * 6 + blue;
        Some(Color::Indexed(c))
    } else if s == "black" {
        Some(Color::Indexed(0))
    } else if s == "red" {
        Some(Color::Indexed(1))
    } else if s == "green" {
        Some(Color::Indexed(2))
    } else if s == "yellow" {
        Some(Color::Indexed(3))
    } else if s == "blue" {
        Some(Color::Indexed(4))
    } else if s == "magenta" {
        Some(Color::Indexed(5))
    } else if s == "cyan" {
        Some(Color::Indexed(6))
    } else if s == "white" {
        Some(Color::Indexed(7))
    } else {
        None
    }
}
