use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputField, InputMode, View};
use crate::models::TaskStatus;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status/Help bar
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);

    match app.view {
        View::Projects => draw_projects_view(frame, app, chunks[1]),
        View::ProjectDetail => draw_project_detail_view(frame, app, chunks[1]),
        View::Tasks => draw_tasks_view(frame, app, chunks[1]),
        View::TaskDetail => draw_task_detail_view(frame, app, chunks[1]),
        View::ExecutionLogs => draw_execution_logs_view(frame, app, chunks[1]),
    }

    draw_status_bar(frame, app, chunks[2]);

    // Draw input popup if in editing mode
    if app.input_mode == InputMode::Editing {
        draw_input_popup(frame, app);
    }

    // Draw help modal if active
    if app.show_help_modal {
        draw_help_modal(frame, app);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = match app.view {
        View::Projects => " iKanban - Projects ".to_string(),
        View::ProjectDetail => {
            if let Some(project) = &app.project_detail {
                format!(" iKanban - {} ", project.name)
            } else {
                " iKanban - Project Details ".to_string()
            }
        }
        View::Tasks => {
            if let Some(project) = app.selected_project() {
                format!(" iKanban - {} ", project.name)
            } else {
                " iKanban - Tasks ".to_string()
            }
        }
        View::TaskDetail => {
            if let Some(task) = &app.task_detail {
                format!(" iKanban - Task: {} ", task.title)
            } else {
                " iKanban - Task Details ".to_string()
            }
        }
        View::ExecutionLogs => {
            if let Some(execution) = app.selected_execution() {
                format!(
                    " iKanban - Execution: {} ({}) ",
                    execution.id.to_string().chars().take(8).collect::<String>(),
                    execution.status
                )
            } else {
                " iKanban - Execution Logs ".to_string()
            }
        }
    };

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(header, area);
}

fn draw_projects_view(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let style = if i == app.selected_project_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let description = project
                .description
                .as_ref()
                .map(|d| format!(" - {}", d))
                .unwrap_or_default();

            ListItem::new(format!("{}{}", project.name, description)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Projects ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(list, area);
}

fn draw_project_detail_view(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(project) = &app.project_detail {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Project name
                Constraint::Min(0),    // Description
                Constraint::Length(3), // Repo Path
                Constraint::Length(3), // Metadata
            ])
            .split(area);

        // Project name
        let name_title = if project.archived {
            " Name (Archived) "
        } else if project.pinned {
            " Name (Pinned) "
        } else {
            " Name "
        };

        let name_block = Block::default()
            .title(name_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let name = Paragraph::new(project.name.as_str())
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .block(name_block);
        frame.render_widget(name, chunks[0]);

        // Description
        let description_text = project.description.as_deref().unwrap_or("No description");
        let desc_block = Block::default()
            .title(" Description ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let description = Paragraph::new(description_text)
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .block(desc_block);
        frame.render_widget(description, chunks[1]);

        // Repo Path
        let repo_text = project.repo_path.as_deref().unwrap_or("No repository path");
        let repo_block = Block::default()
            .title(" Repository Path ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let repo = Paragraph::new(repo_text)
            .style(Style::default().fg(Color::White))
            .block(repo_block);
        frame.render_widget(repo, chunks[2]);

        // Metadata
        let metadata = format!(
            "ID: {} | Created: {} | Updated: {}",
            project.id,
            project.created_at.format("%Y-%m-%d %H:%M"),
            project.updated_at.format("%Y-%m-%d %H:%M")
        );
        let meta_block = Block::default()
            .title(" Metadata ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let meta = Paragraph::new(metadata)
            .style(Style::default().fg(Color::Gray))
            .block(meta_block);
        frame.render_widget(meta, chunks[3]);
    } else {
        let error = Paragraph::new("No project selected")
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(error, area);
    }
}

fn draw_task_detail_view(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(task) = &app.task_detail {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Description
                Constraint::Length(3), // Status & Branch
                Constraint::Length(3), // Metadata
            ])
            .split(area);

        // Title
        let title_block = Block::default()
            .title(" Title ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let title = Paragraph::new(task.title.as_str())
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .block(title_block);
        frame.render_widget(title, chunks[0]);

        // Description
        let description_text = task.description.as_deref().unwrap_or("No description");
        let desc_block = Block::default()
            .title(" Description ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let description = Paragraph::new(description_text)
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .block(desc_block);
        frame.render_widget(description, chunks[1]);

        // Status & Branch
        let status_text = format!("Status: {:?}", task.status);
        let branch_text = task.branch.as_deref().unwrap_or("No branch");
        let working_dir_text = task.working_dir.as_deref().unwrap_or("No working dir");

        let info_text = format!(
            "{} | Branch: {} | Dir: {}",
            status_text, branch_text, working_dir_text
        );

        let info_block = Block::default()
            .title(" Info ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let info = Paragraph::new(info_text)
            .style(Style::default().fg(Color::White))
            .block(info_block);
        frame.render_widget(info, chunks[2]);

        // Metadata
        let metadata = format!(
            "ID: {} | Created: {} | Updated: {}",
            task.id,
            task.created_at.format("%Y-%m-%d %H:%M"),
            task.updated_at.format("%Y-%m-%d %H:%M")
        );
        let meta_block = Block::default()
            .title(" Metadata ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let meta = Paragraph::new(metadata)
            .style(Style::default().fg(Color::Gray))
            .block(meta_block);
        frame.render_widget(meta, chunks[3]);
    } else {
        let error = Paragraph::new("No task selected")
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(error, area);
    }
}

fn draw_tasks_view(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    draw_task_column(frame, app, columns[0], TaskStatus::Todo, "Todo");
    draw_task_column(
        frame,
        app,
        columns[1],
        TaskStatus::InProgress,
        "In Progress",
    );
    draw_task_column(frame, app, columns[2], TaskStatus::Done, "Done");
}

fn draw_task_column(frame: &mut Frame, app: &App, area: Rect, status: TaskStatus, title: &str) {
    let is_selected_column = app.selected_column == status;
    let tasks = app.tasks_in_column(status);

    let items: Vec<ListItem> = tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let style = if is_selected_column && i == app.selected_task_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
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

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.view {
        View::Projects => "j/k: Navigate | Enter: Open | ?: Help",
        View::ProjectDetail => "j/k: Navigate | Enter: Open Tasks | ?: Help",
        View::Tasks => "h/l: Columns | j/k: Navigate | Enter: Details | ?: Help",
        View::TaskDetail => "Esc: Back | ?: Help",
        View::ExecutionLogs => {
            "j/k: Navigate | g/G: Top/Bottom | Enter: Details | s: Stop | r: Refresh | ?: Help"
        }
    };

    let status = if let Some(msg) = &app.status_message {
        Line::from(vec![
            Span::styled(msg, Style::default().fg(Color::Yellow)),
            Span::raw(" | "),
            Span::raw(help_text),
        ])
    } else {
        Line::from(help_text)
    };

    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn draw_input_popup(frame: &mut Frame, app: &App) {
    let title = match app.input_field {
        InputField::ProjectName => {
            if app.view == View::Projects {
                "New Project Name"
            } else {
                "Edit Project Name"
            }
        }
        InputField::ProjectDescription => "Edit Project Description",
        InputField::ProjectRepoPath => "Edit Project Repository Path",
        InputField::TaskTitle => {
            if app.view == View::Tasks {
                "New Task Title"
            } else {
                "Edit Task Title"
            }
        }
        InputField::TaskDescription => "Edit Task Description",
        InputField::None => "Input",
    };

    // Calculate popup size based on content
    let lines: Vec<&str> = if app.input.is_empty() {
        vec![""]
    } else {
        app.input.lines().collect()
    };

    let max_line_width = lines.iter().map(|l| l.len()).max().unwrap_or(10);
    let line_count = lines.len().max(1);

    // Define reasonable limits and convert to u16
    let popup_width_u16 = (max_line_width + 6).min(80).max(30) as u16;
    let popup_height_u16 = (line_count + 4).min(20).max(6) as u16;

    // Convert to percentage for centered_rect
    let term_size = frame.size();
    let percent_x = (popup_width_u16 * 100 / term_size.width).max(1).min(100);
    let percent_y = (popup_height_u16 * 100 / term_size.height).max(1).min(100);

    let area = centered_rect(percent_x, percent_y, frame.area());

    // Clear the background
    frame.render_widget(Clear, area);

    // Create the text with proper styling
    let text = if app.input.is_empty() {
        ratatui::text::Text::raw("")
    } else {
        ratatui::text::Text::from(app.input.clone())
    };

    let input = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .scroll((0, 0)); // Could add scrolling for long content

    frame.render_widget(input, area);

    // Calculate cursor position
    // The cursor should be at the position corresponding to input_cursor_row and input_cursor_col
    let cursor_line = app.input_cursor_row.min(line_count.saturating_sub(1));
    let cursor_col = if cursor_line < lines.len() {
        app.input_cursor_col.min(lines[cursor_line].len())
    } else {
        app.input_cursor_col
    };

    // Position cursor at (area.x + cursor_col + 1, area.y + cursor_line + 1)
    // Add some padding for the block border
    let cursor_x = area.x + 1 + cursor_col as u16;
    let cursor_y = area.y + 1 + cursor_line as u16;

    // Make sure cursor is within the popup bounds
    let cursor_x = cursor_x.min(area.x + area.width - 2);
    let cursor_y = cursor_y.min(area.y + area.height - 2);

    frame.set_cursor_position((cursor_x, cursor_y));
}

fn draw_help_modal(frame: &mut Frame, app: &App) {
    let shortcuts = app.get_keyboard_shortcuts();
    let title = app.get_keyboard_shortcuts_title();

    // Calculate modal size based on content
    let max_key_width = shortcuts
        .iter()
        .map(|(key, _)| key.len())
        .max()
        .unwrap_or(0);
    let max_desc_width = shortcuts
        .iter()
        .map(|(_, desc)| desc.len())
        .max()
        .unwrap_or(0);

    // Calculate required width and height
    let content_width = (max_key_width + max_desc_width + 7) as u16; // +7 for " | " and padding
    let content_height = (shortcuts.len() + 4) as u16; // +4 for title, borders, and footer

    // Get terminal size
    let term_size = frame.size();
    let modal_width = content_width.min(term_size.width - 4).max(40);
    let modal_height = content_height.min(term_size.height - 4).max(10);

    // Calculate centered position
    let x = (term_size.width - modal_width) / 2;
    let y = (term_size.height - modal_height) / 2;

    let area = Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    };

    // Clear the background
    frame.render_widget(Clear, area);

    // Create the list items for shortcuts
    let items: Vec<ListItem> = shortcuts
        .iter()
        .enumerate()
        .map(|(i, (key, desc))| {
            let is_selected = i == app.help_modal_selected;
            let key_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let desc_style = if is_selected {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
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
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(list, area);

    // Draw footer with close hint
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };
    let footer = Paragraph::new(" Press j/k to navigate, Enter or Esc to close ")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default());
    frame.render_widget(footer, footer_area);
}

/// Draw the execution logs view
fn draw_execution_logs_view(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Execution list
            Constraint::Percentage(70), // Log content
        ])
        .split(area);

    // Draw execution list on the left
    draw_execution_list(frame, app, chunks[0]);

    // Draw log content on the right
    draw_log_content(frame, app, chunks[1]);
}

/// Draw the list of executions for the current session
fn draw_execution_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .executions
        .iter()
        .enumerate()
        .map(|(i, execution)| {
            let style = if i == app.selected_execution_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                match execution.status.as_str() {
                    "running" => Style::default().fg(Color::Green),
                    "failed" => Style::default().fg(Color::Red),
                    "killed" => Style::default().fg(Color::DarkGray),
                    _ => Style::default().fg(Color::White),
                }
            };

            let status = execution.status.clone();
            let id_short = execution.id.to_string().chars().take(8).collect::<String>();
            ListItem::new(format!("{} [{}]", id_short, status)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" Executions ({}) ", app.executions.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(list, area);
}

/// Draw the log content for the selected execution
fn draw_log_content(frame: &mut Frame, app: &App, area: Rect) {
    if app.current_execution_logs.is_empty() {
        let no_logs = Paragraph::new("No logs available")
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .title(" Logs ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        frame.render_widget(no_logs, area);
        return;
    }

    // Get visible logs based on line offset
    let visible_area_height = area.height.saturating_sub(2) as usize; // Account for borders
    let logs_to_show = app
        .current_execution_logs
        .iter()
        .skip(app.log_view_line_offset)
        .take(visible_area_height)
        .collect::<Vec<_>>();

    // Build log text with styling
    let mut log_text = Text::default();
    for log in logs_to_show {
        let level_color = match log.level.to_lowercase().as_str() {
            "error" => Color::Red,
            "warn" => Color::Yellow,
            "debug" => Color::DarkGray,
            "trace" => Color::Indexed(108), // A teal/cyan-like color
            _ => Color::White,
        };

        let timestamp = log.timestamp.format("%H:%M:%S").to_string();
        let level = log.level.to_uppercase();

        log_text.extend(vec![Line::from(vec![
            Span::styled(
                format!("[{}] ", timestamp),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:>5} ", level),
                Style::default()
                    .fg(level_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&log.message, Style::default().fg(Color::White)),
        ])]);
    }

    let log_para = Paragraph::new(log_text)
        .block(
            Block::default()
                .title(format!(
                    " Logs ({} total, showing from line {}) ",
                    app.current_execution_logs.len(),
                    app.log_view_line_offset + 1
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true })
        .scroll((0, 0)); // We'll handle scrolling manually via line offset

    frame.render_widget(log_para, area);
}

/// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
