use crate::db::models::{LogEntry, LogType, Session, SessionStatus, Task};
use crate::keyboard::KeyboardState;
use egui;

pub struct SessionView {
    prompt_input: String,
}

impl SessionView {
    pub fn new() -> Self {
        Self {
            prompt_input: String::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        task: Option<&Task>,
        sessions: &[Session],
        current_session: Option<&Session>,
        logs: &[LogEntry],
        keyboard_state: &KeyboardState,
    ) -> SessionViewAction {
        let mut action = SessionViewAction::None;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("◀ Esc")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(100, 100, 100)),
            );
            ui.add_space(10.0);
            if let Some(task) = task {
                ui.heading(egui::RichText::new(&task.title).size(22.0));
            } else {
                ui.heading("Session View");
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let available_height = ui.available_height();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(250.0);
                ui.set_max_width(300.0);
                self.show_session_list(ui, sessions, current_session, keyboard_state);
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.set_min_width(400.0);

                if let Some(session) = current_session {
                    self.show_session_details(ui, session);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    let logs_height = available_height - 250.0;
                    self.show_logs(ui, logs, logs_height.max(100.0));

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    if session.status == SessionStatus::Running {
                        if ui.button("Stop Session (x)").clicked() {
                            action = SessionViewAction::StopSession(session.id.clone());
                        }
                    }
                } else {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No active session")
                                .size(16.0)
                                .color(egui::Color32::from_rgb(120, 120, 120)),
                        );
                        ui.add_space(20.0);

                        ui.label("Enter a prompt to start a new session:");
                        ui.add_space(8.0);

                        let response = ui.add(
                            egui::TextEdit::multiline(&mut self.prompt_input)
                                .desired_width(350.0)
                                .desired_rows(4)
                                .hint_text("Describe the task for the AI agent..."),
                        );

                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if ui.button("Start Session (s)").clicked()
                                || (response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    && ui.input(|i| i.modifiers.ctrl))
                            {
                                if !self.prompt_input.trim().is_empty() {
                                    action =
                                        SessionViewAction::StartSession(self.prompt_input.clone());
                                    self.prompt_input.clear();
                                }
                            }
                            ui.label(
                                egui::RichText::new("Ctrl+Enter to submit")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(100, 100, 100)),
                            );
                        });
                    });
                }
            });
        });

        action
    }

    fn show_session_list(
        &self,
        ui: &mut egui::Ui,
        sessions: &[Session],
        current_session: Option<&Session>,
        keyboard_state: &KeyboardState,
    ) {
        ui.label(egui::RichText::new("Sessions").strong().size(16.0));
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt("session_list_scroll")
            .show(ui, |ui| {
                if sessions.is_empty() {
                    ui.label(
                        egui::RichText::new("No sessions yet")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(120, 120, 120)),
                    );
                    return;
                }

                for (idx, session) in sessions.iter().enumerate() {
                    let is_current = current_session.map_or(false, |cs| cs.id == session.id);
                    let is_selected = idx == keyboard_state.selected_session_index;

                    let frame = if is_current {
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgb(35, 55, 45))
                            .stroke(egui::Stroke::new(
                                2.0,
                                egui::Color32::from_rgb(100, 200, 150),
                            ))
                            .rounding(4.0)
                            .inner_margin(8.0)
                    } else if is_selected {
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgb(35, 45, 65))
                            .stroke(egui::Stroke::new(
                                2.0,
                                egui::Color32::from_rgb(100, 150, 255),
                            ))
                            .rounding(4.0)
                            .inner_margin(8.0)
                    } else {
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgb(28, 28, 32))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 55)))
                            .rounding(4.0)
                            .inner_margin(8.0)
                    };

                    frame.show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let status_color = match session.status {
                                    SessionStatus::Running => {
                                        egui::Color32::from_rgb(100, 255, 150)
                                    }
                                    SessionStatus::Completed => {
                                        egui::Color32::from_rgb(100, 150, 255)
                                    }
                                    SessionStatus::Failed => egui::Color32::from_rgb(255, 100, 100),
                                    SessionStatus::Killed => egui::Color32::from_rgb(200, 100, 100),
                                };
                                ui.colored_label(status_color, "●");
                                ui.label(
                                    egui::RichText::new(&session.id[..8.min(session.id.len())])
                                        .size(13.0),
                                );
                            });

                            ui.label(
                                egui::RichText::new(
                                    session.created_at.format("%m-%d %H:%M").to_string(),
                                )
                                .size(11.0)
                                .color(egui::Color32::from_rgb(100, 100, 100)),
                            );

                            if let Some(branch) = &session.branch_name {
                                ui.label(
                                    egui::RichText::new(branch)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(150, 130, 100)),
                                );
                            }
                        });
                    });

                    ui.add_space(4.0);
                }
            });
    }

    fn show_session_details(&self, ui: &mut egui::Ui, session: &Session) {
        ui.label(egui::RichText::new("Session Details").strong().size(16.0));
        ui.add_space(8.0);

        egui::Grid::new("session_details_grid")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("ID:");
                ui.label(&session.id);
                ui.end_row();

                ui.label("Status:");
                let status_color = match session.status {
                    SessionStatus::Running => egui::Color32::from_rgb(100, 255, 150),
                    SessionStatus::Completed => egui::Color32::from_rgb(100, 150, 255),
                    SessionStatus::Failed => egui::Color32::from_rgb(255, 100, 100),
                    SessionStatus::Killed => egui::Color32::from_rgb(200, 100, 100),
                };
                ui.colored_label(status_color, session.status.to_string());
                ui.end_row();

                ui.label("Executor:");
                ui.label(&session.executor_type);
                ui.end_row();

                if let Some(branch) = &session.branch_name {
                    ui.label("Branch:");
                    ui.label(branch);
                    ui.end_row();
                }

                if let Some(worktree) = &session.worktree_path {
                    ui.label("Worktree:");
                    ui.label(worktree.display().to_string());
                    ui.end_row();
                }

                ui.label("Started:");
                ui.label(session.created_at.format("%Y-%m-%d %H:%M:%S").to_string());
                ui.end_row();

                if let Some(finished) = session.finished_at {
                    ui.label("Finished:");
                    ui.label(finished.format("%Y-%m-%d %H:%M:%S").to_string());
                    ui.end_row();
                }
            });
    }

    fn show_logs(&self, ui: &mut egui::Ui, logs: &[LogEntry], height: f32) {
        ui.label(egui::RichText::new("Execution Logs").strong().size(16.0));
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .id_salt("session_logs_scroll")
            .max_height(height)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if logs.is_empty() {
                    ui.label(
                        egui::RichText::new("No logs yet")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(120, 120, 120)),
                    );
                    return;
                }

                for log in logs {
                    let (icon, color) = match log.log_type {
                        LogType::Stdout => ("▶", egui::Color32::from_rgb(100, 150, 255)),
                        LogType::Stderr => ("✖", egui::Color32::from_rgb(255, 100, 100)),
                        LogType::Event => ("⚙", egui::Color32::from_rgb(150, 150, 150)),
                    };

                    ui.horizontal(|ui| {
                        ui.colored_label(color, icon);
                        ui.label(
                            egui::RichText::new(log.timestamp.format("%H:%M:%S").to_string())
                                .size(11.0)
                                .color(egui::Color32::from_rgb(100, 100, 100)),
                        );
                        ui.label(&log.content);
                    });
                    ui.add_space(2.0);
                }
            });
    }

    pub fn get_prompt(&self) -> &str {
        &self.prompt_input
    }

    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt_input = prompt;
    }

    pub fn clear_prompt(&mut self) {
        self.prompt_input.clear();
    }
}

impl Default for SessionView {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionViewAction {
    None,
    StartSession(String),
    StopSession(String),
    SelectSession(String),
}
