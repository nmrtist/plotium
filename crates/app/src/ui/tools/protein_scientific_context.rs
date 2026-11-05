use super::*;
use plotium_core::protein_context::{
    Experiment, ExperimentAxisRole, ExperimentKind, SampleCondition, SampleConditionId,
    ScientificContext,
};

#[derive(Clone)]
pub(super) struct ContextDraft {
    baseline: ScientificContext,
    context: ScientificContext,
    dataset: Option<DatasetId>,
    sample: Option<SampleConditionId>,
    json: Option<String>,
}
impl ContextDraft {
    fn new(app: &PlotiumApp, dataset: Option<DatasetId>) -> Self {
        let dataset = dataset.filter(|&d| protein::volume(&app.doc, d).is_ok());
        let context = app.doc.protein.context.clone();
        let sample = dataset
            .and_then(|d| context.experiments.get(&d).map(|e| e.sample))
            .or_else(|| context.samples.first().map(|s| s.id));
        Self {
            baseline: context.clone(),
            context,
            dataset,
            sample,
            json: None,
        }
    }
    fn missing_target(&self, app: &PlotiumApp) -> bool {
        self.dataset
            .is_some_and(|d| protein::volume(&app.doc, d).is_err())
    }
    fn conflicted(&self, app: &PlotiumApp) -> bool {
        self.missing_target(app) || self.baseline != app.doc.protein.context
    }
    fn keep_samples_only(&mut self) {
        if let Some(dataset) = self.dataset.take() {
            self.context.experiments.remove(&dataset);
        }
    }
    fn save(&self, app: &mut PlotiumApp) -> Result<(), String> {
        if self.missing_target(app) {
            return Err(
                "The draft spectrum was deleted. Continue with sample conditions only or reload."
                    .into(),
            );
        }
        if self.json.is_some() {
            return Err("Apply or cancel advanced JSON edits before saving.".into());
        }
        app.save_scientific_context_draft(&self.baseline, self.context.clone())
    }
}

pub(super) fn scientific_context(
    app: &mut PlotiumApp,
    ui: &mut Ui,
    panel: &mut Panel,
    active: Option<DatasetId>,
) {
    ui.collapsing("Scientific context", |ui| {
        ui.label("Initial scope: single standard-residue chain, single major conformer, dense 3D spectra.");
        if let Some(dataset) = active {
            ui.label(format!("Scope check: {}", app.doc.protein.context
                .check_structure_scope(&app.doc, dataset)
                .map_or_else(|e| e, |()| "Within initial scope; scientific evidence is not yet validated".into())));
        }
        if panel.context_draft.is_none() && ui.button("Edit sample conditions and experiments").clicked() {
            panel.context_draft = Some(ContextDraft::new(app, active));
        }
        let Some(draft) = &mut panel.context_draft else { return };
        if draft.conflicted(app) {
            if draft.missing_target(app) {
                ui.label("The draft spectrum was deleted. Reload, or keep sample edits and continue without this spectrum. Apply or cancel JSON edits first.");
                if ui.add_enabled(draft.json.is_none(), egui::Button::new("Keep sample edits (sample-only)")).clicked() {
                    draft.keep_samples_only();
                    panel.feedback.clear();
                }
            } else {
                ui.label("Scientific context changed since editing began. Saving is blocked. Reload current context or discard this draft.");
            }
            if ui.button("Reload current context (discard draft)").clicked() {
                *draft = ContextDraft::new(app, draft.dataset);
                panel.feedback.clear();
            }
        }
        if let Some(dataset) = draft.dataset {
            ui.label(format!("Editing spectrum: {}", spectrum_name(app, dataset)));
        } else {
            ui.label("Editing sample conditions only. Select a 3D spectrum before opening a new draft to edit its experiment.");
        }
        let advanced = draft.json.is_some();
        ui.add_enabled_ui(!advanced, |ui| form(ui, draft));
        if draft.json.is_none() && ui.button("Advanced JSON").clicked() {
            match serde_json::to_string_pretty(&draft.context) {
                Ok(text) => draft.json = Some(text),
                Err(e) => panel.feedback = e.to_string(),
            }
        }
        if let Some(text) = &mut draft.json {
            ui.label("Advanced edits affect all sample and experiment records. Apply JSON to the draft before saving.");
            ui.add(egui::TextEdit::multiline(text).code_editor().desired_rows(14));
            if ui.button("Apply JSON to draft").clicked() {
                match serde_json::from_str::<ScientificContext>(text)
                    .map_err(|e| e.to_string())
                    .and_then(|context| { context.validate(&app.doc)?; Ok(context) })
                {
                    Ok(context) => {
                        draft.context = context;
                        draft.sample = draft.dataset.and_then(|d| draft.context.experiments.get(&d).map(|e| e.sample))
                            .or_else(|| draft.context.samples.first().map(|s| s.id));
                        draft.json = None;
                        panel.feedback.clear();
                    }
                    Err(e) => panel.feedback = e,
                }
            }
            if ui.button("Cancel JSON edits").clicked() { draft.json = None; }
        }
        let save = ui.add_enabled(!draft.conflicted(app) && draft.json.is_none(), egui::Button::new("Save scientific context")).clicked();
        if save {
            match draft.save(app) {
                Ok(()) => { panel.context_draft = None; panel.feedback.clear(); }
                Err(e) => panel.feedback = e,
            }
        }
        if ui.button("Discard context edits").clicked() {
            panel.context_draft = None;
            panel.feedback.clear();
        }
    });
}

fn form(ui: &mut Ui, draft: &mut ContextDraft) {
    ui.label("Reuse a sample only for identical conditions. Editing a shared sample affects every linked spectrum.");
    ComboBox::from_id_salt("context_sample")
        .selected_text(
            draft
                .sample
                .and_then(|id| draft.context.samples.iter().find(|s| s.id == id))
                .map_or("Select sample", |s| s.name.as_str()),
        )
        .show_ui(ui, |ui| {
            for (i, sample) in draft.context.samples.iter().enumerate() {
                ui.selectable_value(
                    &mut draft.sample,
                    Some(sample.id),
                    format!("{} ({})", sample.name, i + 1),
                );
            }
        });
    if ui.button("New sample conditions").clicked() {
        let sample = SampleCondition {
            name: format!("Sample conditions {}", draft.context.samples.len() + 1),
            ..Default::default()
        };
        draft.sample = Some(sample.id);
        draft.context.samples.push(sample);
    }
    if let Some(sample) = draft
        .sample
        .and_then(|id| draft.context.samples.iter_mut().find(|s| s.id == id))
    {
        field(ui, "Sample name", &mut sample.name);
        optional_number(ui, "Temperature (K)", &mut sample.temperature_kelvin, 298.0);
        optional_number(ui, "pH (0–14)", &mut sample.ph, 7.0);
        field(ui, "Solvent", &mut sample.solvent);
        field(ui, "Isotope labeling", &mut sample.isotope_labeling);
        field(ui, "Construct", &mut sample.construct);
        field(
            ui,
            "Nonstandard chemistry / ligands / special bonding",
            &mut sample.nonstandard_chemistry,
        );
        ui.checkbox(
            &mut sample.single_major_conformer,
            "Reviewed: single major conformer",
        );
    }
    let Some(dataset) = draft.dataset else { return };
    let Some(sample) = draft.sample else {
        ui.label("Select or create sample conditions to record this experiment.");
        return;
    };
    let experiment = draft
        .context
        .experiments
        .entry(dataset)
        .or_insert(Experiment {
            sample,
            kind: ExperimentKind::Unknown,
            roles: Default::default(),
            source: String::new(),
        });
    experiment.sample = sample;
    ComboBox::from_id_salt("context_experiment")
        .selected_text(kind_name(experiment.kind))
        .show_ui(ui, |ui| {
            for kind in [
                ExperimentKind::Unknown,
                ExperimentKind::Noesy,
                ExperimentKind::Hnca,
                ExperimentKind::Hnco,
            ] {
                ui.selectable_value(&mut experiment.kind, kind, kind_name(kind));
            }
        });
    ui.label("Axis roles follow acquisition F1/F2/F3 order. Acquired nuclei remain unchanged.");
    for (axis, role) in experiment.roles.iter_mut().enumerate() {
        ComboBox::from_id_salt(("context_role", axis))
            .selected_text(format!("F{}: {}", axis + 1, role_name(*role)))
            .show_ui(ui, |ui| {
                for value in [
                    ExperimentAxisRole::Unknown,
                    ExperimentAxisRole::ObservedProton,
                    ExperimentAxisRole::IndirectProton,
                    ExperimentAxisRole::Nitrogen,
                    ExperimentAxisRole::Carbon,
                ] {
                    ui.selectable_value(role, value, role_name(value));
                }
            });
    }
    field(ui, "Experiment metadata source", &mut experiment.source);
}
fn field(ui: &mut Ui, label: &str, text: &mut String) {
    ui.label(label);
    ui.text_edit_singleline(text);
}
fn optional_number(ui: &mut Ui, label: &str, value: &mut Option<f64>, initial: f64) {
    let mut known = value.is_some();
    if ui.checkbox(&mut known, label).changed() {
        *value = known.then_some(initial);
    }
    if let Some(value) = value {
        ui.add(DragValue::new(value).speed(0.1));
    }
}
fn kind_name(kind: ExperimentKind) -> &'static str {
    match kind {
        ExperimentKind::Unknown => "Unknown experiment",
        ExperimentKind::Noesy => "NOESY",
        ExperimentKind::Hnca => "HNCA",
        ExperimentKind::Hnco => "HNCO",
    }
}
fn role_name(role: ExperimentAxisRole) -> &'static str {
    match role {
        ExperimentAxisRole::Unknown => "Unknown role",
        ExperimentAxisRole::ObservedProton => "Observed proton",
        ExperimentAxisRole::IndirectProton => "Indirect proton",
        ExperimentAxisRole::Nitrogen => "Nitrogen",
        ExperimentAxisRole::Carbon => "Carbon",
    }
}

#[cfg(test)]
#[path = "protein_scientific_context_tests.rs"]
mod tests;
