mod parser;

use std::path::PathBuf;

use chrono::{Local, NaiveDate};
use eframe::egui::{self, Button, CentralPanel, ScrollArea, TextEdit, Widget};
use egui_extras::DatePickerButton;
use once_cell::sync::Lazy;
use rfd::FileDialog;

static DEFAULT_DATE: Lazy<NaiveDate> = Lazy::new(|| Local::now().date_naive());

fn main() -> eframe::Result {
    eframe::run_native(
        "mycampus-calendar-rs",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::<App>::default())),
    )
}

#[derive(Default)]
struct App {
    data: String,
    excluded_dates: Vec<ExcludedDate>,
    output_folder: Option<PathBuf>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered_justified(|ui| ui.heading("mycampus-calendar-rs"));
            ui.separator();

            ui.add_space(12.0);
            ui.heading("MyOntarioTech Schedule Data");

            ScrollArea::vertical()
                .max_height(100.0)
                .animated(false)
                .show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        TextEdit::multiline(&mut self.data)
                            .hint_text("Paste the copied schedule data here."),
                    )
                });

            ui.add_space(12.0);
            ui.heading("Excluded Dates");

            ui.horizontal(|ui| {
                if ui.button("➕ Single").clicked() {
                    self.excluded_dates.push(ExcludedDate::single());
                }

                if ui.button("➕ Range").clicked() {
                    self.excluded_dates.push(ExcludedDate::range());
                }
            });

            ui.add_space(6.0);

            let mut i = 0;
            self.excluded_dates.retain_mut(|range| {
                ui.horizontal(|ui| {
                    let should_delete = ui.button("❌").clicked();

                    date_picker(ui, &mut range.start, &format!("{i}_start"));

                    if let Some(end) = &mut range.end {
                        ui.label("-");
                        date_picker(ui, end, &format!("{i}_end"));
                    }

                    i += 1;
                    !should_delete
                })
                .inner
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                if ui.button("Select output folder...").clicked() {
                    if let Some(path) = FileDialog::new().pick_folder() {
                        self.output_folder = Some(path);
                    }
                }

                if let Some(path) = &self.output_folder {
                    ui.label(path.display().to_string());
                }
            });

            ui.add_space(12.0);

            let enabled = !self.data.is_empty() && self.output_folder.is_some();
            ui.add_enabled(enabled, Button::new("Generate"));
        });
    }
}

fn date_picker(ui: &mut egui::Ui, selection: &mut NaiveDate, id_source: &str) -> egui::Response {
    DatePickerButton::new(selection)
        .id_source(id_source)
        .calendar_week(false)
        .show_icon(false)
        .ui(ui)
}

#[derive(Debug, Clone)]
struct ExcludedDate {
    start: NaiveDate,
    end: Option<NaiveDate>,
}

impl ExcludedDate {
    fn single() -> Self {
        Self {
            start: *DEFAULT_DATE,
            end: None,
        }
    }

    fn range() -> Self {
        Self {
            start: *DEFAULT_DATE,
            end: Some(*DEFAULT_DATE),
        }
    }
}
