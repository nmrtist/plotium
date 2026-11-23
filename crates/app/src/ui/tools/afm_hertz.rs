use super::afm_fit::command;
use crate::ui::commands::AfmFitCommand as Fit;
use egui::Ui;
use egui_phosphor::regular as icon;
use plotium_analysis::afm_hertz::{HertzResult, HertzWindow};
use plotium_core::state::{AfmDeflectionSource, AfmHertzReference, AfmHertzWindow};
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
            .ok_or("Show an AFM force plot before opening Hertz Fit.");
    }
    let draft = app
        .session
        .ui
        .afm_hertz_task
        .as_ref()
        .ok_or("Open Hertz Fit first.")?;
    match choice {
        Fit::SuggestHertz => {
            app.afm_hertz_calibration(draft.target)?;
            draft.hertz_suggestion_radius().map(|_| ())
        }
        Fit::ApplyHertzSuggestion => {
            app.afm_hertz_calibration(draft.target)?;
            draft.hertz_suggestion_radius()?;
            app.afm_hertz_suggestion()
                .map(|_| ())
                .ok_or("Suggest inputs for the current curve first.")
        }
        Fit::RunHertz => {
            let k = app.afm_hertz_calibration(draft.target)?;
            draft.radius()?;
            draft
                .options(k)
                .map(|_| ())
                .map_err(|_| "Complete the fit settings and window limits.")
        }
        Fit::HertzDataSheet => app
            .afm_hertz_result()
            .map(|_| ())
            .ok_or("Calculate a Hertz fit with the current inputs."),
        _ => Err("Choose a Hertz Fit command."),
    }
}

pub(super) fn run_command(app: &mut PlotiumApp, choice: Fit) {
    if choice == Fit::OpenHertz {
        let Some(target) = active_target(app) else {
            return;
        };
        if app
            .session
            .ui
            .afm_hertz_task
            .as_ref()
            .is_none_or(|task| task.target != target)
        {
            let tip_radius_nm = app
                .doc
                .dataset_by_id(target.source)
                .and_then(|dataset| dataset.as_afm())
                .and_then(|afm| afm.data.forces.as_ref())
                .and_then(|forces| forces.tip_radius_nm);
            app.session.ui.afm_hertz_task = Some(AfmHertzDraft::new(target, tip_radius_nm));
        }
        let ui = &mut app.session.ui;
        ui.afm_fit_task = None;
        ui.curve_fit_task_dataset = None;
        ui.curve_fit_task_collapsed = false;
        ui.open_task_tab(TaskDockTab::CurveFit);
        return;
    }
    let suggestion_action = matches!(choice, Fit::SuggestHertz | Fit::ApplyHertzSuggestion);
    let result = match choice {
        Fit::SuggestHertz => app.suggest_afm_hertz(),
        Fit::ApplyHertzSuggestion => app.apply_afm_hertz_suggestion(),
        Fit::RunHertz => app.run_afm_hertz(),
        Fit::HertzDataSheet => app.create_afm_hertz_output(),
        _ => return,
    };
    if let Some(task) = &mut app.session.ui.afm_hertz_task {
        let error = result.as_ref().err().cloned();
        if suggestion_action {
            task.suggestion_error = error;
        } else {
            task.error = error;
        }
    }
    if let Err(error) = result {
        app.session.status = error;
    }
}

pub(super) fn body(app: &mut PlotiumApp, ui: &mut Ui, target: AfmFitTarget) {
    ui.separator();
    ui.weak("Approach / Extend · Spherical contact");
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
    if let Ok(spring) = calibration
        && let Some(context) = app
            .doc
            .dataset_by_id(target.source)
            .and_then(|data| data.as_afm())
            .and_then(|afm| afm.force_curve_context())
    {
        ui.small(format!("Spring constant (file): {spring} N/m"));
        match context.deflection_source {
            AfmDeflectionSource::StoredLength => {
                ui.small("Deflection: calibrated length from file");
            }
            AfmDeflectionSource::VoltageSensitivity => {
                if let Some(sensitivity) = context.deflection_sensitivity_m_per_v {
                    ui.small(format!(
                        "Deflection sensitivity (file): {} nm/V",
                        sensitivity * 1e9
                    ));
                }
            }
            _ => {}
        }
    }
    let selection = app.afm_fit_selection(target);

    if let Some(draft) = &mut app.session.ui.afm_hertz_task {
        let mut changed = false;
        let method = |linearized| {
            if linearized {
                "Linearized Hertz"
            } else {
                "Manual Contact Hertz"
            }
        };
        let previous = (draft.linearized, draft.window, draft.reference);
        egui::ComboBox::from_id_salt("hertz_method")
            .selected_text(method(draft.linearized))
            .show_ui(ui, |ui| {
                for linearized in [false, true] {
                    ui.selectable_value(&mut draft.linearized, linearized, method(linearized));
                }
            });
        egui::Grid::new("hertz_inputs").show(ui, |ui| {
            changed |= number_field(ui, "Probe radius (nm)", &mut draft.radius_nm);
            ui.end_row();
            if !draft.linearized {
                changed |= number_field(ui, "Contact Z (nm)", &mut draft.contact_z_nm);
                ui.end_row();
            }
        });
        changed |= range_field(ui, "Baseline Z (nm)", &mut draft.baseline_z_nm, selection);
        egui::ComboBox::from_id_salt("hertz_window")
            .selected_text(draft.window.label())
            .show_ui(ui, |ui| {
                for window in AfmHertzWindow::ALL {
                    ui.selectable_value(&mut draft.window, window, window.label());
                }
            });
        changed |= match draft.window {
            AfmHertzWindow::RelativeZ => {
                range_field(ui, "Fit Z (nm)", &mut draft.fit_z_nm, selection)
            }
            AfmHertzWindow::ForceNn => {
                range_field(ui, "Fit force (nN)", &mut draft.fit_force_nn, None)
            }
            AfmHertzWindow::ForcePercent => {
                range_field(ui, "Fit force (%)", &mut draft.fit_force_percent, None)
            }
        };
        let reference_label = |reference| match reference {
            AfmHertzReference::Zero => "Off (corrected zero)",
            AfmHertzReference::Automatic => "Automatic",
            AfmHertzReference::Manual => "Manual value",
        };
        ui.horizontal_wrapped(|ui| {
            ui.label("Minimum force reference");
            egui::ComboBox::from_id_salt("hertz_reference")
                .selected_text(reference_label(draft.reference))
                .show_ui(ui, |ui| {
                    for reference in [
                        AfmHertzReference::Zero,
                        AfmHertzReference::Automatic,
                        AfmHertzReference::Manual,
                    ] {
                        ui.selectable_value(
                            &mut draft.reference,
                            reference,
                            reference_label(reference),
                        );
                    }
                });
        });
        if draft.reference == AfmHertzReference::Manual {
            changed |= number_field(ui, "Reference force (nN)", &mut draft.reference_nn);
        }
        changed |= previous != (draft.linearized, draft.window, draft.reference);
        if changed {
            draft.error = None;
            draft.suggestion_error = None;
        }
    }
    ui.horizontal_wrapped(|ui| {
        command(app, ui, Fit::SuggestHertz, false);
        command(app, ui, Fit::ApplyHertzSuggestion, false);
    });
    if let Some(message) = app
        .session
        .ui
        .afm_hertz_task
        .as_ref()
        .and_then(|draft| draft.suggestion_error.as_deref())
    {
        ui.colored_label(ui.visuals().warn_fg_color, message);
    }
    if let Some(candidate) = app.afm_hertz_suggestion() {
        let limits =
            |[lo, hi]: [f64; 2]| format!("{} to {} nm", preview::value(lo), preview::value(hi));
        ui.strong("Suggested inputs (not applied)");
        ui.small(format!(
            "Contact Z: {} nm",
            preview::value(candidate.input.contact_z_nm)
        ));
        ui.small(format!(
            "Baseline Z: {}",
            limits(candidate.input.baseline_z_nm)
        ));
        ui.small(format!("Fit Z: {}", limits(candidate.input.fit_z_nm)));
    }
    if let Ok(spring) = calibration {
        let contact = egui::CollapsingHeader::new("Approach preview")
            .default_open(true)
            .show(ui, |ui| {
                let draft = app.session.ui.afm_hertz_task.as_ref()?;
                match app.afm_hertz_points(target) {
                    Ok(points) => preview::approach(
                        ui,
                        &points,
                        draft,
                        spring,
                        app.afm_hertz_suggestion().map(|candidate| candidate.input),
                    ),
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
        if let Some(range) = selection
            && ui
                .button(icon::SELECTION)
                .on_hover_text("Use source plot selection")
                .clicked()
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
    let method = if let Some(zero) = result.extrapolated_zero_z_nm {
        ui.small(format!(
            "Fitted extrapolated zero: {} nm",
            preview::value(zero)
        ));
        "Linearized Hertz"
    } else {
        "Manual Contact Hertz"
    };
    let (window, [low, high]) = match result.options.window {
        HertzWindow::RelativeZ(bounds) => (AfmHertzWindow::RelativeZ, bounds),
        HertzWindow::ForceNn(bounds) => (AfmHertzWindow::ForceNn, bounds),
        HertzWindow::ForcePercent(bounds) => (AfmHertzWindow::ForcePercent, bounds),
    };
    ui.small(format!(
        "{method} · {} · {} to {}",
        window.label(),
        preview::value(low),
        preview::value(high)
    ));
    ui.small(format!("Reference force: {:.6} nN", result.reference_nn));
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
    } else if result.extrapolated_zero_z_nm.is_some() {
        ui.small("Indentation is measured from the fitted extrapolated zero.");
    } else {
        ui.small("Contact sensitivity unavailable");
    }
    preview::result(ui, result);
}
