use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
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
    }

    draw_status_bar(frame, app, chunks[2]);

    // Draw input popup if in editing mode
    if app.input_mode == InputMode::Editing {
        draw_input_popup(frame, app);
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
        View::Projects => {
            "q: Quit | n: New Project | d: Delete | Enter: Open | j/k: Navigate"
        }
        View::ProjectDetail => {
            "Esc: Back | Enter: Open Tasks | e: Edit Name | d: Edit Description | j/k: Navigate"
        }
        View::Tasks => {
            "Esc: Back | n: New Task | d: Delete | Space: Move Status | h/l: Columns | j/k: Navigate"
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
        InputField::TaskTitle => "New Task Title",
        InputField::None => "Input",
    };

    let area = centered_rect(60, 20, frame.area());

    // Clear the background
    frame.render_widget(Clear, area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    frame.render_widget(input, area);

    // Set cursor position
    frame.set_cursor_position((area.x + app.input.len() as u16 + 1, area.y + 1));
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
