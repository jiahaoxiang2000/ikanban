use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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
                    if !handle_normal_mode(app, key.code).await? {
                        break;
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

async fn handle_normal_mode(app: &mut App, key: KeyCode) -> anyhow::Result<bool> {
    match app.view {
        View::Projects => match key {
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
            KeyCode::Char('r') => {
                if let Err(e) = app.load_projects().await {
                    app.set_status(&format!("Error: {}", e));
                } else {
                    app.set_status("Refreshed");
                }
            }
            _ => {}
        },
        View::ProjectDetail => match key {
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
        View::Tasks => match key {
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
            KeyCode::Char('r') => {
                if let Some(project) = app.selected_project() {
                    let project_id = project.id;
                    if let Err(e) = app.load_tasks(project_id).await {
                        app.set_status(&format!("Error: {}", e));
                    } else {
                        app.set_status("Refreshed");
                    }
                }
            }
            KeyCode::Enter => {
                app.enter_task_detail_view();
            }
            _ => {}
        },
        View::TaskDetail => match key {
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
