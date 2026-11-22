use super::afm_fit::command;
use crate::ui::commands::AfmFitCommand as Fit;
use egui::Ui;
use egui_phosphor::regular as icon;
use plotium_analysis::afm_hertz::HertzResult;
use plotium_core::state::{
    AfmFitTarget, AfmForceBranch, AfmHertzDraft, AxisRange, PlotiumApp, TaskDockTab,
};

#[path = "afm_hertz_preview.rs"]
mod preview;

fn active_target(app: &PlotiumApp) -> Option<AfmFitTarget> {
    let source = app
        .active_dataset()
        .and_then(|di| app.doc.datasets.get(di))?;
    app.afm_fit_target(source.resource_id(), AfmForceBranch::Approach)
}

pub(super) fn gate(app: &PlotiumApp, choice: Fit) -> Result<(), &'static str> {
    if choice == Fit::OpenHertz {
        return active_target(app)
            .map(|_| ())
            .ok_or("Show an AFM force plot before opening Manual Hertz.");
    }
    let draft = app
        .session
        .ui
        .afm_hertz_task
        .as_ref()
        .ok_or("Open Manual Hertz first.")?;
    match choice {
        Fit::RunHertz => {
            let k = app.afm_hertz_calibration(draft.target)?;
            if !draft.spring_confirmed {
                return Err("Confirm the file spring constant.");
            }
            let input = draft
                .input(k)
                .map_err(|_| "Enter a radius, a contact Z, and both window limits.")?;
            (input.radius_nm > 0.0)
                .then_some(())
                .ok_or("Enter a positive probe radius.")
        }
        Fit::HertzDataSheet => app
            .afm_hertz_result()
            .map(|_| ())
            .ok_or("Calculate a Hertz fit with the current inputs."),
        _ => Err("Choose a Manual Hertz command."),
    }
}

pub(super) fn run_command(app: &mut PlotiumApp, choice: Fit) {
    if choice == Fit::OpenHertz {
        let Some(target) = active_target(app) else {
            return;
        };
        let ui = &mut app.session.ui;
        if ui
            .afm_hertz_task
            .as_ref()
            .is_none_or(|task| task.target != target)
        {
            ui.afm_hertz_task = Some(AfmHertzDraft::new(target));
        }
        ui.afm_fit_task = None;
        ui.curve_fit_task_dataset = None;
        ui.curve_fit_task_collapsed = false;
        ui.open_task_tab(TaskDockTab::CurveFit);
        return;
    }
    let result = match choice {
        Fit::RunHertz => app.run_afm_hertz(),
        Fit::HertzDataSheet => app.create_afm_hertz_output(),
        _ => return,
    };
    if let Some(task) = &mut app.session.ui.afm_hertz_task {
        task.error = result.as_ref().err().cloned();
    }
    if let Err(error) = result {
        app.session.status = error;
    }
}

pub(super) fn body(app: &mut PlotiumApp, ui: &mut Ui, target: AfmFitTarget) {
    ui.separator();
    ui.weak("Nonadhesive spherical contact");
    if let Some(afm) = app
        .doc
        .dataset_by_id(target.source)
        .and_then(|data| data.as_afm())
        && afm
            .data
            .forces
            .as_ref()
            .is_some_and(|f| f.grid_width > 1 || f.grid_height > 1)
    {
        let [col, row] = afm.selected_pixel;
        ui.small(format!("Pixel: column {col}, row {row}"));
    }
    let calibration = app.afm_hertz_calibration(target);
    if let Err(message) = calibration {
        ui.colored_label(ui.visuals().warn_fg_color, message);
    }
    let selection = app.afm_fit_selection(target);

    if let Some(draft) = &mut app.session.ui.afm_hertz_task {
        let mut changed = ui
            .add_enabled_ui(calibration.is_ok(), |ui| {
                ui.checkbox(&mut draft.spring_confirmed, "Use file spring constant")
                    .changed()
            })
            .inner;
        egui::Grid::new("hertz_inputs").show(ui, |ui| {
            changed |= number_field(ui, "Probe radius (nm)", &mut draft.radius_nm);
            ui.end_row();
            changed |= number_field(ui, "Contact Z (nm)", &mut draft.contact_z_nm);
            ui.end_row();
        });
        changed |= range_field(ui, "Baseline Z (nm)", &mut draft.baseline_z_nm, selection);
        changed |= range_field(ui, "Fit Z (nm)", &mut draft.fit_z_nm, selection);
        if changed {
            draft.error = None;
        }
    }
    if calibration.is_ok() {
        let contact = egui::CollapsingHeader::new("Approach preview")
            .default_open(true)
            .show(ui, |ui| {
                let draft = app.session.ui.afm_hertz_task.as_ref()?;
                match app.afm_hertz_points(target) {
                    Ok(points) => preview::approach(ui, &points, draft),
                    Err(error) => {
                        ui.small(error);
                        None
                    }
                }
            })
            .body_returned
            .flatten();
        if let Some(z) = contact
            && let Some(draft) = &mut app.session.ui.afm_hertz_task
        {
            draft.contact_z_nm = z.to_string();
            draft.error = None;
        }
    }

    ui.separator();
    command(app, ui, Fit::RunHertz, true);
    if let Some(error) = app
        .session
        .ui
        .afm_hertz_task
        .as_ref()
        .and_then(|draft| draft.error.as_deref())
    {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    if let Some(result) = app.afm_hertz_result() {
        summary(ui, result);
    } else {
        ui.weak("No current result. Calculate a Hertz fit.");
    }
    command(app, ui, Fit::HertzDataSheet, false);
}

fn text_box(ui: &mut Ui, value: &mut String, hint: &str) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(66.0)
            .hint_text(hint),
    )
}

fn number_field(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    let label = ui.label(label);
    text_box(ui, value, "").labelled_by(label.id).changed()
}

fn range_field(
    ui: &mut Ui,
    label: &str,
    limits: &mut [String; 2],
    selection: Option<AxisRange>,
) -> bool {
    let label = ui.label(label);
    ui.horizontal_wrapped(|ui| {
        let [minimum, maximum] = limits;
        let changed = text_box(ui, minimum, "Min").labelled_by(label.id).changed()
            | text_box(ui, maximum, "Max").labelled_by(label.id).changed();
        let copy = ui
            .add_enabled(selection.is_some(), egui::Button::new(icon::SELECTION))
            .on_hover_text("Use source plot selection")
            .on_disabled_hover_text("Select a range on the source plot.");
        if copy.clicked()
            && let Some(range) = selection
        {
            *minimum = range.min.to_string();
            *maximum = range.max.to_string();
            return true;
        }
        changed
    })
    .inner
}

fn summary(ui: &mut Ui, result: &HertzResult) {
    crate::typography::headline_label(
        ui,
        format!(
            "Reduced modulus E*: {} MPa",
            preview::value(result.reduced_modulus_mpa)
        ),
    );
    ui.small(format!(
        "Baseline RMS: {} nN ({} points)",
        preview::value(result.baseline_rms_nn),
        result.baseline_points
    ));
    ui.small(format!(
        "Residual RMS: {} nN ({} points)",
        preview::value(result.residual_rms_nn),
        result.rows.len()
    ));
    let ratio = format!(
        "Maximum indentation / radius: {}",
        preview::value(result.max_indentation_over_radius)
    );
    if result.max_indentation_over_radius >= 0.1 {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!("{ratio} (check shallow indentation)"),
        );
    } else {
        ui.small(ratio);
    }
    if let Some(sensitivity) = &result.contact_sensitivity {
        let modulus = |value: Option<f64>| {
            value.map_or_else(
                || "unavailable".into(),
                |mpa| format!("{} MPa", preview::value(mpa)),
            )
        };
        let shift = preview::value(sensitivity.shift_nm);
        ui.small(format!(
            "Contact sensitivity at +/- {shift} nm (fixed windows, not a confidence interval)"
        ));
        ui.small(format!(
            "-{shift} nm: {} | +{shift} nm: {}",
            modulus(sensitivity.lower_contact_modulus_mpa),
            modulus(sensitivity.higher_contact_modulus_mpa),
        ));
    } else {
        ui.small("Contact sensitivity unavailable");
    }
    preview::result(ui, result);
}
