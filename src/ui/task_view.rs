use crate::db::models::{Project, Task, TaskStatus};
use crate::keyboard::KeyboardState;
use crate::ui::Column;
use egui;

pub struct TaskView {
    todo_column: Column,
    in_progress_column: Column,
    in_review_column: Column,
    done_column: Column,
}

impl TaskView {
    pub fn new() -> Self {
        Self {
            todo_column: Column::new(TaskStatus::Todo),
            in_progress_column: Column::new(TaskStatus::InProgress),
            in_review_column: Column::new(TaskStatus::InReview),
            done_column: Column::new(TaskStatus::Done),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        project: Option<&Project>,
        tasks: &[Task],
        keyboard_state: &KeyboardState,
    ) -> Option<String> {
        let mut selected_task_id = None;

        ui.horizontal(|ui| {
            if let Some(project) = project {
                ui.label(
                    egui::RichText::new("◀ Esc")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(100, 100, 100)),
                );
                ui.add_space(10.0);
                ui.heading(egui::RichText::new(&project.name).size(22.0));
                ui.label(
                    egui::RichText::new(format!("({})", project.path.display()))
                        .size(12.0)
                        .color(egui::Color32::from_rgb(120, 120, 120)),
                );
            } else {
                ui.heading("Task Board");
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let available_width = ui.available_width();
        let column_width = (available_width - 48.0) / 4.0;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(column_width);
                ui.set_max_width(column_width);
                let is_selected = keyboard_state.selected_column == 0;
                if let Some(task_id) =
                    self.todo_column
                        .show(ui, tasks, is_selected, keyboard_state.selected_row)
                {
                    selected_task_id = Some(task_id);
                }
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.set_min_width(column_width);
                ui.set_max_width(column_width);
                let is_selected = keyboard_state.selected_column == 1;
                if let Some(task_id) = self.in_progress_column.show(
                    ui,
                    tasks,
                    is_selected,
                    keyboard_state.selected_row,
                ) {
                    selected_task_id = Some(task_id);
                }
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.set_min_width(column_width);
                ui.set_max_width(column_width);
                let is_selected = keyboard_state.selected_column == 2;
                if let Some(task_id) =
                    self.in_review_column
                        .show(ui, tasks, is_selected, keyboard_state.selected_row)
                {
                    selected_task_id = Some(task_id);
                }
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.set_min_width(column_width);
                ui.set_max_width(column_width);
                let is_selected = keyboard_state.selected_column == 3;
                if let Some(task_id) =
                    self.done_column
                        .show(ui, tasks, is_selected, keyboard_state.selected_row)
                {
                    selected_task_id = Some(task_id);
                }
            });
        });

        selected_task_id
    }

    pub fn get_selected_task<'a>(
        &self,
        tasks: &'a [Task],
        keyboard_state: &KeyboardState,
    ) -> Option<&'a Task> {
        let status = match keyboard_state.selected_column {
            0 => TaskStatus::Todo,
            1 => TaskStatus::InProgress,
            2 => TaskStatus::InReview,
            3 => TaskStatus::Done,
            _ => return None,
        };

        let column_tasks: Vec<&Task> = tasks.iter().filter(|t| t.status == status).collect();
        column_tasks.get(keyboard_state.selected_row).copied()
    }
}

impl Default for TaskView {
    fn default() -> Self {
        Self::new()
    }
}
