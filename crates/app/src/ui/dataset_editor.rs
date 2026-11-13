use std::collections::BTreeMap;

use egui::{Context, Ui};
use plotium_core::state::{DatasetId, OrganizationEdit, PlotiumApp};

#[derive(Clone)]
struct Editor {
    targets: Vec<DatasetId>,
    experiment: String,
    tag: String,
}

fn key(app: &PlotiumApp) -> egui::Id {
    egui::Id::new(("dataset_metadata", app.session.project_context.to_string()))
}

pub(super) fn open(app: &PlotiumApp, ctx: &Context, dataset: Option<DatasetId>) {
    let targets: Vec<_> = if let Some(dataset) = dataset {
        vec![dataset]
    } else {
        app.session
            .ui
            .data_selection
            .iter()
            .filter_map(|&index| app.doc.datasets.get(index))
            .map(|d| d.resource_id())
            .collect()
    };
    let experiment = if let [id] = targets.as_slice() {
        app.doc
            .dataset_index(*id)
            .and_then(|i| app.doc.datasets[i].experiment_label())
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    };
    ctx.data_mut(|data| {
        data.insert_temp(
            key(app),
            Editor {
                targets,
                experiment,
                tag: String::new(),
            },
        )
    });
}

pub(super) fn window(app: &mut PlotiumApp, ctx: &Context) {
    let id = key(app);
    let Some(mut editor) = ctx.data(|data| data.get_temp::<Editor>(id)) else {
        return;
    };
    editor
        .targets
        .retain(|&id| app.doc.dataset_index(id).is_some());
    let mut open = !editor.targets.is_empty();
    if open {
        egui::Window::new("Edit experiment and tags")
            .id(id)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| editor.render(app, ui));
            });
    }
    ctx.data_mut(|data| {
        if open {
            data.insert_temp(id, editor);
        } else {
            data.remove::<Editor>(id);
        }
    });
}

impl Editor {
    fn render(&mut self, app: &mut PlotiumApp, ui: &mut Ui) {
        let count = self.targets.len();
        let targets: Vec<_> = self
            .targets
            .iter()
            .filter_map(|&id| app.doc.dataset_index(id))
            .map(|i| &app.doc.datasets[i])
            .collect();
        let Some(first) = targets.first() else { return };
        ui.heading(if count == 1 {
            first.browser_name()
        } else {
            format!("{count} datasets")
        });
        let label = first.experiment_label();
        let mixed = targets
            .iter()
            .any(|dataset| dataset.experiment_label() != label);
        ui.label(if mixed {
            "Current experiment: Mixed values".into()
        } else {
            format!("Current experiment: {}", label.unwrap_or("None"))
        });
        let imported = (count == 1).then(|| {
            first
                .acquisition_identity()
                .acquisition
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_owned()
        });
        if let Some(imported) = &imported {
            ui.weak(format!(
                "Imported name: {}",
                if imported.is_empty() {
                    "None"
                } else {
                    imported
                }
            ));
        }
        let current = (count == 1).then(|| label.unwrap_or_default().to_owned());
        let has_override = targets
            .iter()
            .any(|d| d.organization().experiment_override().is_some());
        let suggest_nmr = targets.iter().any(|d| d.compact_category() == "NMR");
        let mut tags = BTreeMap::<String, (String, usize)>::new();
        for dataset in &targets {
            for tag in dataset.organization().tags() {
                tags.entry(tag.to_lowercase())
                    .or_insert_with(|| (tag.clone(), 0))
                    .1 += 1;
            }
        }
        let label = ui.label("Experiment display name");
        ui.horizontal(|ui| {
            let field = ui
                .text_edit_singleline(&mut self.experiment)
                .labelled_by(label.id);
            if suggest_nmr {
                egui::ComboBox::from_id_salt("experiment_suggestions")
                    .selected_text("Suggestions")
                    .show_ui(ui, |ui| {
                        for name in ["COSY", "HMQC", "HNCA", "HNCO", "HSQC", "NOESY", "TOCSY"] {
                            if ui.button(name).clicked() {
                                self.experiment = name.into();
                                ui.close();
                            }
                        }
                    });
            }
            let enter = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            let value = self.experiment.trim();
            let can_set = !value.is_empty() && current.as_deref() != Some(value);
            if (ui
                .add_enabled(can_set, egui::Button::new("Set name"))
                .clicked()
                || enter)
                && can_set
            {
                app.edit_dataset_organization(
                    &self.targets,
                    OrganizationEdit::Experiment(Some(&self.experiment)),
                );
            }
        });
        if ui
            .add_enabled(has_override, egui::Button::new("Use imported names"))
            .clicked()
        {
            app.edit_dataset_organization(&self.targets, OrganizationEdit::Experiment(None));
            self.experiment = imported.unwrap_or_default();
        }
        ui.separator();
        let label = ui.label("Add tag");
        let mut project_tags = BTreeMap::new();
        let query = self.tag.trim().to_lowercase();
        for dataset in &app.doc.datasets {
            for tag in dataset.organization().tags() {
                let key = tag.to_lowercase();
                if key.contains(&query) {
                    project_tags.entry(key).or_insert_with(|| tag.clone());
                }
            }
        }
        ui.horizontal(|ui| {
            let field = ui.text_edit_singleline(&mut self.tag).labelled_by(label.id);
            egui::ComboBox::from_id_salt("tag_completion")
                .selected_text("Project tags")
                .show_ui(ui, |ui| {
                    for tag in project_tags.values() {
                        if ui.button(tag).clicked() {
                            self.tag.clone_from(tag);
                            ui.close();
                        }
                    }
                    if project_tags.is_empty() {
                        ui.weak("No matching tags");
                    }
                });
            let enter = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if ui
                .add_enabled(!self.tag.trim().is_empty(), egui::Button::new("Add"))
                .clicked()
                || enter
            {
                app.edit_dataset_organization(&self.targets, OrganizationEdit::AddTag(&self.tag));
                self.tag.clear();
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (tag, uses) in tags.values() {
                let label = if *uses == count {
                    format!("Remove {tag}")
                } else {
                    format!("Remove {tag} ({uses}/{count})")
                };
                if ui.small_button(label).clicked() {
                    app.edit_dataset_organization(&self.targets, OrganizationEdit::RemoveTag(tag));
                }
            }
        });
    }
}
