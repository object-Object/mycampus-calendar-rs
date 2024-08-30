mod parser;

use std::{collections::HashSet, panic, path::PathBuf};

use chrono::{Local, NaiveDate};
use eframe::egui::{self, Button, CentralPanel, ScrollArea, TextEdit, Widget};
use egui_extras::DatePickerButton;
use once_cell::sync::Lazy;
use rfd::FileDialog;

const OUTPUT_FOLDER_KEY: &str = "output_folder";

static DEFAULT_DATE: Lazy<NaiveDate> = Lazy::new(|| Local::now().date_naive());

fn main() -> eframe::Result {
    let app_name = "mycampus-calendar-rs";
    eframe::run_native(
        app_name,
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            let app = match cc.storage {
                Some(storage) => App {
                    output_folder: storage
                        .get_string(OUTPUT_FOLDER_KEY)
                        .map(|s| s.into())
                        .take_if(|p: &mut PathBuf| p.is_dir()),
                    ..Default::default()
                },
                None => App::default(),
            };
            Ok(Box::<App>::new(app))
        }),
    )
}

#[derive(Default)]
struct App {
    data: String,
    excluded_dates: Vec<ExcludedDate>,
    output_folder: Option<PathBuf>,
    result_text: Option<String>,
}

impl App {
    fn can_generate_calendars(&self) -> bool {
        !self.data.is_empty() && self.output_folder.is_some()
    }

    fn generate_calendars(&mut self) {
        if let Some(output_folder) = &self.output_folder {
            let exdate = self
                .excluded_dates
                .iter()
                .flat_map(|d| d.iter_days())
                .collect::<HashSet<_>>();

            // FIXME: this should really return a result instead of catching errors.
            let result =
                panic::catch_unwind(|| parser::generate(output_folder, &self.data, exdate));

            self.result_text = Some(match result {
                Ok(n) if n > 0 => format!("☑ Generated {n} calendar(s)."),
                Ok(_) => "⚠ No calendars were generated.".to_owned(),
                Err(_) => {
                    "⚠ An error occurred while generating calendars. See console for more details."
                        .to_owned()
                }
            });
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered_justified(|ui| ui.heading("mycampus-calendar-rs"));
            ui.separator();
            ui.hyperlink_to(
                "Usage instructions",
                "https://github.com/object-Object/mycampus-calendar-rs",
            );

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

            if !self.excluded_dates.is_empty() {
                ui.add_space(6.0);
            }

            let mut i = 0;
            self.excluded_dates.retain_mut(|range| {
                ui.horizontal(|ui| {
                    let should_delete = ui.button("❌").clicked();

                    if date_picker(ui, &mut range.start, &format!("{i}_start")).changed()
                        && !range.was_changed
                        && range.end.is_some()
                    {
                        range.was_changed = true;
                        range.end = Some(range.start);
                    };

                    if let Some(end) = &mut range.end {
                        ui.label("-");
                        if date_picker(ui, end, &format!("{i}_end")).changed() && !range.was_changed
                        {
                            range.was_changed = true;
                            range.start = *end;
                        };
                    }

                    i += 1;
                    !should_delete
                })
                .inner
            });

            ui.add_space(12.0);
            ui.heading("Output");

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

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.can_generate_calendars(),
                        Button::new("Generate calendar files"),
                    )
                    .clicked()
                {
                    self.generate_calendars();
                }

                if let Some(result_text) = &self.result_text {
                    ui.label(result_text);
                }
            });
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(output_folder) = self
            .output_folder
            .as_ref()
            .take_if(|p| p.is_dir())
            .and_then(|p| p.to_str())
        {
            storage.set_string(OUTPUT_FOLDER_KEY, output_folder.to_owned())
        }
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
    was_changed: bool,
}

impl ExcludedDate {
    fn new(start: NaiveDate, end: Option<NaiveDate>) -> Self {
        Self {
            start,
            end,
            was_changed: false,
        }
    }

    fn single() -> Self {
        Self::new(*DEFAULT_DATE, None)
    }

    fn range() -> Self {
        Self::new(*DEFAULT_DATE, Some(*DEFAULT_DATE))
    }

    fn iter_days(&self) -> impl Iterator<Item = NaiveDate> {
        let (start, end) = match self.end {
            Some(end) if end < self.start => (end, self.start),
            Some(end) => (self.start, end),
            None => (self.start, self.start),
        };
        start
            .iter_days()
            .take_while(move |&next| next <= end)
            .take(365) // sanity check - surely no one would want to exclude a whole year.... right?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_days() {
        do_test((2020, 1, 1), None, vec![(2020, 1, 1)]);
        do_test((2020, 1, 1), Some((2020, 1, 1)), vec![(2020, 1, 1)]);
        do_test(
            (2020, 1, 1),
            Some((2020, 1, 2)),
            vec![(2020, 1, 1), (2020, 1, 2)],
        );
        do_test(
            (2020, 1, 2),
            Some((2020, 1, 1)),
            vec![(2020, 1, 1), (2020, 1, 2)],
        );
        do_test(
            (2020, 1, 1),
            Some((2020, 1, 3)),
            vec![(2020, 1, 1), (2020, 1, 2), (2020, 1, 3)],
        );
        do_test(
            (2020, 1, 3),
            Some((2020, 1, 1)),
            vec![(2020, 1, 1), (2020, 1, 2), (2020, 1, 3)],
        );
    }

    fn do_test(start: (i32, u32, u32), end: Option<(i32, u32, u32)>, want: Vec<(i32, u32, u32)>) {
        let input = ExcludedDate::new(
            NaiveDate::from_ymd_opt(start.0, start.1, start.2).unwrap(),
            end.map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap()),
        );

        let want_dates = want
            .iter()
            .map(|&(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(input.iter_days().collect::<Vec<_>>(), want_dates);
    }
}
