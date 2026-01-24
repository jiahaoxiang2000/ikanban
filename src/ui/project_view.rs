use crate::db::models::Project;
use crate::keyboard::KeyboardState;
use egui;

pub struct ProjectView {}

impl ProjectView {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        projects: &[Project],
        keyboard_state: &KeyboardState,
    ) -> Option<String> {
        let mut selected_project_id = None;

        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading(egui::RichText::new("Select Project").size(28.0));
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Choose a repository to work with")
                    .size(14.0)
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
            ui.add_space(20.0);
        });

        let available_width = ui.available_width();
        let card_width = (available_width - 60.0).min(800.0);

        ui.vertical_centered(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("project_view_scroll")
                .show(ui, |ui| {
                    if projects.is_empty() {
                        ui.add_space(50.0);
                        ui.label(
                            egui::RichText::new("No projects yet")
                                .size(18.0)
                                .color(egui::Color32::from_rgb(120, 120, 120)),
                        );
                        ui.add_space(10.0);
                        ui.label("Press 'n' to create a new project");
                        return;
                    }

                    for (idx, project) in projects.iter().enumerate() {
                        let is_selected = idx == keyboard_state.selected_project_index;

                        let frame = if is_selected {
                            egui::Frame::default()
                                .fill(egui::Color32::from_rgb(35, 45, 65))
                                .stroke(egui::Stroke::new(
                                    2.0,
                                    egui::Color32::from_rgb(100, 150, 255),
                                ))
                                .rounding(8.0)
                                .inner_margin(16.0)
                        } else {
                            egui::Frame::default()
                                .fill(egui::Color32::from_rgb(28, 28, 32))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 55)))
                                .rounding(8.0)
                                .inner_margin(16.0)
                        };

                        ui.allocate_ui_with_layout(
                            egui::vec2(card_width, 0.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                frame.show(ui, |ui| {
                                    ui.set_min_width(card_width - 32.0);

                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                if is_selected {
                                                    ui.label(
                                                        egui::RichText::new("▶").size(16.0).color(
                                                            egui::Color32::from_rgb(100, 150, 255),
                                                        ),
                                                    );
                                                }
                                                ui.label(
                                                    egui::RichText::new(&project.name)
                                                        .strong()
                                                        .size(18.0),
                                                );
                                            });

                                            ui.add_space(6.0);

                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}",
                                                    project.path.display()
                                                ))
                                                .size(13.0)
                                                .color(egui::Color32::from_rgb(130, 130, 130)),
                                            );

                                            ui.add_space(4.0);

                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Created: {}",
                                                    project.created_at.format("%Y-%m-%d %H:%M")
                                                ))
                                                .size(12.0)
                                                .color(egui::Color32::from_rgb(100, 100, 100)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if is_selected {
                                                    ui.label(
                                                        egui::RichText::new("Enter to open")
                                                            .size(12.0)
                                                            .color(egui::Color32::from_rgb(
                                                                100, 150, 255,
                                                            )),
                                                    );
                                                }
                                            },
                                        );
                                    });

                                    let response = ui.interact(
                                        ui.min_rect(),
                                        ui.id().with(idx),
                                        egui::Sense::click(),
                                    );
                                    if response.clicked() {
                                        selected_project_id = Some(project.id.clone());
                                    }
                                });
                            },
                        );

                        ui.add_space(8.0);
                    }
                });
        });

        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("j/k - navigate | Enter - select | n - new project | q - quit")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(100, 100, 100)),
            );
        });

        selected_project_id
    }
}

impl Default for ProjectView {
    fn default() -> Self {
        Self::new()
    }
}
