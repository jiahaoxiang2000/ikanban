use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use ikanban_tui::app::{App, InputField, InputMode, View};
use ikanban_tui::ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get server URL from environment or use default
    let server_url = std::env::var("IKANBAN_SERVER")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(&server_url);

    // Initial data load
    if let Err(e) = app.load_projects().await {
        app.set_status(&format!("Failed to connect: {}", e));
    } else {
        // Connect to projects WebSocket for real-time updates
        app.connect_projects_ws();
    }

    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        // Process any pending WebSocket events
        if let Err(e) = app.process_ws_events().await {
            app.set_status(&format!("WebSocket error: {}", e));
        }

        terminal.draw(|f| ui::draw(f, app))?;

        // Poll for events with timeout to allow async operations
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            // Only handle key press events (not release)
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.input_mode {
                InputMode::Normal => {
                    // Handle help modal separately from normal mode
                    if app.show_help_modal {
                        if !handle_help_modal(app, key.code).await? {
                            break;
                        }
                    } else {
                        if !handle_normal_mode(app, key.code, key.modifiers).await? {
                            break;
                        }
                    }
                }
                InputMode::Editing => {
                    handle_editing_mode(app, key.code).await?;
                }
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}

async fn handle_normal_mode(app: &mut App, key_code: KeyCode, key_modifiers: KeyModifiers) -> anyhow::Result<bool> {
    // Check for Ctrl+R (refresh)
    if key_code == KeyCode::Char('r') && key_modifiers.contains(KeyModifiers::CONTROL) {
        if let Err(e) = app.load_projects().await {
            app.set_status(&format!("Error: {}", e));
        } else {
            app.set_status("Refreshed");
        }
        return Ok(true);
    }

    // Check for ? key (toggle help modal)
    if key_code == KeyCode::Char('?') {
        app.toggle_help_modal();
        return Ok(true);
    }

    match app.view {
        View::Projects => match key_code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('j') | KeyCode::Down => app.next_project(),
            KeyCode::Char('k') | KeyCode::Up => app.previous_project(),
            KeyCode::Enter => {
                if let Err(e) = app.enter_task_view().await {
                    app.set_status(&format!("Error: {}", e));
                }
            }
            KeyCode::Char('e') => {
                app.enter_project_detail_view();
            }
            KeyCode::Char('n') => {
                app.start_input(InputField::ProjectName);
            }
            KeyCode::Char('d') => {
                if let Err(e) = app.delete_selected_project().await {
                    app.set_status(&format!("Error: {}", e));
                } else {
                    app.set_status("Project deleted");
                }
            }
            _ => {}
        },
        View::ProjectDetail => match key_code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.enter_project_view();
            }
            KeyCode::Enter => {
                if let Err(e) = app.enter_task_view().await {
                    app.set_status(&format!("Error: {}", e));
                }
            }
            KeyCode::Char('e') => {
                if let Some(project) = &app.project_detail {
                    app.input = project.name.clone();
                    app.start_input(InputField::ProjectName);
                }
            }
            KeyCode::Char('d') => {
                if let Some(project) = &app.project_detail {
                    app.input = project.description.as_deref().unwrap_or("").to_string();
                    app.start_input(InputField::ProjectDescription);
                }
            }
            KeyCode::Char('r') => {
                if let Some(project) = &app.project_detail {
                    app.input = project.repo_path.as_deref().unwrap_or("").to_string();
                    app.start_input(InputField::ProjectRepoPath);
                }
            }
            _ => {}
        },
        View::Tasks => match key_code {
            KeyCode::Esc => {
                app.enter_project_view();
            }
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('j') | KeyCode::Down => app.next_task(),
            KeyCode::Char('k') | KeyCode::Up => app.previous_task(),
            KeyCode::Char('h') | KeyCode::Left => app.previous_column(),
            KeyCode::Char('l') | KeyCode::Right => app.next_column(),
            KeyCode::Char(' ') => {
                if let Err(e) = app.move_task_to_next_status().await {
                    app.set_status(&format!("Error: {}", e));
                }
            }
            KeyCode::Char('n') => {
                app.start_input(InputField::TaskTitle);
            }
            KeyCode::Char('d') => {
                if let Err(e) = app.delete_selected_task().await {
                    app.set_status(&format!("Error: {}", e));
                } else {
                    app.set_status("Task deleted");
                }
            }
            KeyCode::Enter => {
                app.enter_task_detail_view();
            }
            _ => {}
        },
        View::TaskDetail => match key_code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if let Err(e) = app.enter_task_view().await {
                    app.set_status(&format!("Error: {}", e));
                }
            }
            KeyCode::Char('e') => {
                if let Some(task) = &app.task_detail {
                    app.input = task.title.clone();
                    app.start_input(InputField::TaskTitle);
                }
            }
            KeyCode::Char('d') => {
                if let Some(task) = &app.task_detail {
                    app.input = task.description.as_deref().unwrap_or("").to_string();
                    app.start_input(InputField::TaskDescription);
                }
            }
            _ => {}
        },
    }

    Ok(true)
}

async fn handle_editing_mode(app: &mut App, key: KeyCode) -> anyhow::Result<()> {
    match key {
        KeyCode::Enter => {
            if let Err(e) = app.submit_input().await {
                app.set_status(&format!("Error: {}", e));
            }
        }
        KeyCode::Esc => {
            app.cancel_input();
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        _ => {}
    }

    Ok(())
}

async fn handle_help_modal(app: &mut App, key: KeyCode) -> anyhow::Result<bool> {
    let shortcuts = app.get_keyboard_shortcuts();
    let max_index = shortcuts.len().saturating_sub(1);

    match key {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
            app.close_help_modal();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if max_index > 0 {
                app.help_modal_selected = (app.help_modal_selected + 1).min(max_index);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if max_index > 0 {
                app.help_modal_selected = app.help_modal_selected.saturating_sub(1);
            }
        }
        _ => {}
    }

    Ok(true)
}
