use super::task_card::{self, TaskCardGeometry};
use crate::ui::commands::{self, AfmFitCommand as Fit, CommandId};
use egui::Ui;
use plotium_analysis::fit_model::{FitModelDefinition, FitSolver, RobustLoss, WeightMode};
use plotium_analysis::models::{LINEAR_ID, builtin_models};
use plotium_core::state::{
    AfmDeflectionSource, AfmFitDraft, AfmFitTarget, AfmForceBranch, AfmForceCurveContext,
    AfmForceFit, AfmForceXSource, FrameRef, PlotiumApp, TaskDockTab,
};

const TAB: TaskDockTab = TaskDockTab::CurveFit;

pub(in crate::ui) fn open_task(app: &mut PlotiumApp, branch: AfmForceBranch) {
    let Some(source) = app.active_dataset().and_then(|di| app.doc.datasets.get(di)) else {
        return;
    };
    let Some(target) = app.afm_fit_target(source.resource_id(), branch) else {
        return;
    };
    app.session.ui.afm_hertz_task = None;
    let result = app.afm_fit_result(target);
    app.session.ui.afm_fit_task = Some(AfmFitDraft {
        target,
        model: result.map_or_else(|| LINEAR_ID.into(), |fit| fit.model.id.clone()),
        options: result.map_or_else(Default::default, |fit| fit.options.clone()),
        overlay: result.is_none_or(|fit| fit.shown),
        error: None,
    });
    match plotium_core::fit_model_library::FitModelLibrary::load() {
        Ok(library) => app.session.ui.fit_custom_models = library.models,
        Err(error) => {
            app.session.ui.fit_custom_models.clear();
            app.session.status = format!("Could not load custom fit models: {error}");
        }
    }
    app.session.ui.curve_fit_task_dataset = None;
    app.session.ui.curve_fit_task_collapsed = false;
    app.session.ui.open_task_tab(TAB);
}

pub(super) fn target(app: &PlotiumApp) -> Option<AfmFitTarget> {
    let ui = &app.session.ui;
    ui.afm_hertz_task
        .as_ref()
        .map(|draft| draft.target)
        .or_else(|| ui.afm_fit_task.as_ref().map(|draft| draft.target))
}

pub(super) fn switch_to_table(app: &mut PlotiumApp, di: usize) {
    let ui = &mut app.session.ui;
    ui.afm_fit_task = None;
    ui.afm_hertz_task = None;
    ui.curve_fit_task_dataset = Some(di);
    ui.open_task_tab(TAB);
}

pub(in crate::ui) fn afm_fit_gate(app: &PlotiumApp, command: Fit) -> Result<(), &'static str> {
    match command {
        Fit::OpenHertz
        | Fit::SuggestHertz
        | Fit::ApplyHertzSuggestion
        | Fit::RunHertz
        | Fit::HertzDataSheet => {
            return super::afm_hertz::gate(app, command);
        }
        Fit::ShowSource => {
            return target(app)
                .and_then(|target| app.afm_fit_plot(target))
                .map(|_| ())
                .ok_or("Open AFM Force Fit on an available source plot.");
        }
        _ => {}
    }
    let draft = app
        .session
        .ui
        .afm_fit_task
        .as_ref()
        .ok_or("Open AFM Force Fit first.")?;
    app.afm_fit_plot(draft.target)
        .ok_or("Open AFM Force Fit on an available source plot.")?;
    if command == Fit::Run {
        return Ok(());
    }
    let result = app
        .afm_fit_result(draft.target)
        .ok_or("Run AFM Fit to create a result.")?;
    if matches!(command, Fit::ToggleOverlay | Fit::Duplicate) && !result.converged {
        return Err("Run a converged fit before displaying its fitted curve.");
    }
    Ok(())
}

pub(in crate::ui) fn run_command(app: &mut PlotiumApp, command: Fit) {
    match command {
        Fit::OpenHertz
        | Fit::SuggestHertz
        | Fit::ApplyHertzSuggestion
        | Fit::RunHertz
        | Fit::HertzDataSheet => {
            super::afm_hertz::run_command(app, command);
            return;
        }
        Fit::ShowSource => {
            if let Some(target) = target(app) {
                show_source(app, target);
            }
            return;
        }
        _ => {}
    }
    let Some(draft) = app.session.ui.afm_fit_task.clone() else {
        return;
    };
    let target = draft.target;
    let result = match command {
        Fit::Run => app
            .run_afm_fit(target, &draft.model, draft.options, draft.overlay)
            .map(|()| {
                if app.session.active_canvas != app.doc.canvas_index(target.canvas) {
                    show_source(app, target);
                }
            }),
        Fit::ToggleOverlay => app.toggle_afm_fit_overlay(target).map(|()| {
            let shown = app.afm_fit_result(target).is_some_and(|fit| fit.shown);
            if let Some(task) = &mut app.session.ui.afm_fit_task {
                task.overlay = shown;
            }
        }),
        Fit::ZoomRange => app.zoom_to_afm_fit_range(target).map(|()| {
            if app.session.active_canvas != app.doc.canvas_index(target.canvas) {
                show_source(app, target);
            }
        }),
        Fit::Duplicate => app.duplicate_afm_fit_plot(target),
        Fit::ResidualPlot => app.create_afm_fit_output(target, true),
        Fit::DataSheet => app.create_afm_fit_output(target, false),
        Fit::OpenHertz
        | Fit::SuggestHertz
        | Fit::ApplyHertzSuggestion
        | Fit::RunHertz
        | Fit::HertzDataSheet
        | Fit::ShowSource => return,
    };
    if let Some(task) = &mut app.session.ui.afm_fit_task {
        task.error = result.as_ref().err().cloned();
    }
    if let Err(error) = result {
        app.session.status = error;
    }
}

fn show_source(app: &mut PlotiumApp, target: AfmFitTarget) {
    if let Some(ci) = app.doc.canvas_index(target.canvas) {
        app.doc.canvases[ci].selected_object = Some(target.object);
        app.reveal_board_frame(FrameRef::Page(ci));
    }
}

pub(super) fn render_task(app: &mut PlotiumApp, host: &mut Ui) {
    if !task_card::is_active(app, TAB) {
        return;
    }
    let Some(target) = target(app) else {
        return;
    };
    let title = if app.session.ui.afm_hertz_task.is_some() {
        "Manual Hertz"
    } else {
        "AFM Force Fit"
    };
    let collapsed = app.session.ui.curve_fit_task_collapsed;
    let TaskCardGeometry {
        pos,
        width,
        body_height,
    } = task_card::geometry(app, host, TAB, 310.0, collapsed);
    let area_id = task_card::area_id(TAB);
    let dark = host.visuals().dark_mode;
    let mut close = false;
    let mut collapse = false;
    task_card::area(host, area_id, pos).show(host.ctx(), |ui| {
        ui.set_width(width);
        crate::ui::card_frame(dark, egui::Margin::ZERO).show(ui, |ui| {
            task_card::header(
                ui,
                area_id,
                title,
                Some(target.branch.name().to_owned()),
                |ui| {
                    close = crate::ui::panel_chrome::close(ui, &format!("Close {title}")).clicked();
                    collapse = crate::ui::panel_chrome::collapse(ui, collapsed, title).clicked();
                },
            );
            if task_card::tab_bar(app, TAB, ui) {
                ui.separator();
            }
            if !collapsed {
                ui.separator();
                task_card::sized_body(ui, body_height, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| body(app, ui));
                });
            }
        });
        if !collapsed {
            task_card::resize_handles(app, ui, area_id, TAB, width, body_height);
        }
    });
    if close {
        app.session.ui.close_task_tab(TAB);
    } else if collapse {
        app.session.ui.curve_fit_task_collapsed = !collapsed;
    }
}

fn body(app: &mut PlotiumApp, ui: &mut Ui) {
    let Some(target) = target(app) else {
        return;
    };
    let Some((source, context)) = app
        .afm_fit_plot(target)
        .and_then(|_| app.doc.dataset_by_id(target.source))
        .and_then(|source| Some((source, source.as_afm()?.force_curve_context()?)))
    else {
        ui.small("Source plot unavailable. Open AFM Force Fit from a force curve.");
        return;
    };
    ui.weak(source.display_name());
    let x = context.x.label();
    let x_note = if context.x_source == AfmForceXSource::ReconstructedZ {
        ", not indentation"
    } else {
        ""
    };
    ui.small(format!("X: {x}{x_note} | Y: {}", context.y.label()));
    calibration(ui, &context);
    if app.session.active_canvas != app.doc.canvas_index(target.canvas) {
        command(app, ui, Fit::ShowSource, false);
    }
    if app.session.ui.afm_hertz_task.is_some() {
        super::afm_hertz::body(app, ui, target);
        return;
    }
    if let Some(range) = app.afm_fit_selection(target) {
        ui.label(format!(
            "Selected {x}: {:.4} to {:.4}",
            range.min, range.max
        ));
    } else {
        ui.label("Range: Entire branch");
    }
    let Some(draft) = &mut app.session.ui.afm_fit_task else {
        return;
    };
    let models = builtin_models()
        .iter()
        .chain(&app.session.ui.fit_custom_models)
        .filter(|model| model.independent_variables.len() == 1 && model.responses.len() == 1);
    let model_name = |model: &FitModelDefinition| model.name.as_str().to_owned();
    ui.horizontal_wrapped(|ui| {
        ui.label("Empirical model");
        let selected = models
            .clone()
            .find(|model| model.id == draft.model)
            .map(model_name)
            .unwrap_or_else(|| draft.model.clone());
        egui::ComboBox::from_id_salt("afm_fit_model")
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for model in models {
                    ui.selectable_value(&mut draft.model, model.id.clone(), &model.name);
                }
            });
    });
    ui.checkbox(&mut draft.overlay, "Overlay on source plot");
    ui.collapsing("Fit Options", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("Solver");
            egui::ComboBox::from_id_salt("afm_fit_solver")
                .selected_text(match draft.options.solver {
                    FitSolver::BoundedTrustRegion => "Bounded trust region",
                    FitSolver::LevenbergMarquardt => "Levenberg-Marquardt",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut draft.options.solver,
                        FitSolver::BoundedTrustRegion,
                        "Bounded trust region",
                    );
                    ui.selectable_value(
                        &mut draft.options.solver,
                        FitSolver::LevenbergMarquardt,
                        "Levenberg-Marquardt",
                    );
                });
        });
        ui.horizontal_wrapped(|ui| {
            let choices = [
                (WeightMode::Auto, "Auto"),
                (WeightMode::Equal, "Equal"),
                (WeightMode::Relative, "Relative"),
                (WeightMode::Poisson, "Poisson"),
            ];
            let selected = choices
                .iter()
                .find(|(mode, _)| *mode == draft.options.weights)
                .map_or("Custom", |(_, label)| *label);
            ui.label("Weights");
            egui::ComboBox::from_id_salt("afm_fit_weights")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (mode, label) in choices {
                        ui.selectable_value(&mut draft.options.weights, mode, label);
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Robust loss");
            let choices = [
                (RobustLoss::None, "None"),
                (RobustLoss::Huber(1.0), "Huber"),
                (RobustLoss::SoftL1(1.0), "Soft-L1"),
                (RobustLoss::Cauchy(1.0), "Cauchy"),
            ];
            let selected = choices
                .iter()
                .find(|(value, _)| *value == draft.options.robust_loss)
                .map_or("Custom", |(_, label)| *label);
            egui::ComboBox::from_id_salt("afm_fit_loss")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (value, label) in choices {
                        ui.selectable_value(&mut draft.options.robust_loss, value, label);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Starts");
            ui.add(egui::DragValue::new(&mut draft.options.multi_start).range(1..=32));
        });
    });
    command(app, ui, Fit::Run, true);
    if let Some(error) = app
        .session
        .ui
        .afm_fit_task
        .as_ref()
        .and_then(|task| task.error.as_ref())
    {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    ui.separator();
    if let Some(result) = app.afm_fit_result(target).cloned() {
        summary(ui, &result);
        ui.horizontal_wrapped(|ui| {
            command(app, ui, Fit::ZoomRange, false);
            ui.menu_button("Output", |ui| {
                for choice in [
                    Fit::ToggleOverlay,
                    Fit::Duplicate,
                    Fit::ResidualPlot,
                    Fit::DataSheet,
                ] {
                    if command(app, ui, choice, false) {
                        ui.close();
                    }
                }
            });
        });
    } else {
        ui.weak("No result. Run AFM Fit.");
    }
}

fn quantity_label(name: &str, unit: &str) -> String {
    if unit.is_empty() {
        name.to_owned()
    } else {
        format!("{name} ({unit})")
    }
}

fn calibration(ui: &mut Ui, context: &AfmForceCurveContext<'_>) {
    let sensitivity = context.deflection_sensitivity_m_per_v.map(|value| {
        let nm_per_v = value * 1.0e9;
        if (0.001..1000.0).contains(&nm_per_v) {
            format!("{nm_per_v:.4} nm/V")
        } else {
            format!("{value:.4e} m/V")
        }
    });
    let deflection = match context.deflection_source {
        AfmDeflectionSource::StoredLength => sensitivity.map_or_else(
            || "Deflection: stored length".to_owned(),
            |value| format!("Deflection: stored length (file sensitivity {value}, not reapplied)"),
        ),
        AfmDeflectionSource::VoltageSensitivity => sensitivity.map_or_else(
            || "Deflection: converted from voltage".to_owned(),
            |value| format!("Deflection: file sensitivity {value} applied"),
        ),
        AfmDeflectionSource::MissingVoltageSensitivity => {
            "Deflection unavailable: missing or invalid voltage sensitivity".to_owned()
        }
        AfmDeflectionSource::UnsupportedSignalUnit => {
            "Deflection unavailable: unrecognized signal units".to_owned()
        }
    };
    ui.small(deflection);
    let converts_deflection = matches!(
        context.deflection_source,
        AfmDeflectionSource::StoredLength | AfmDeflectionSource::VoltageSensitivity
    );
    let spring = match (context.spring_constant_n_per_m, converts_deflection) {
        (Some(value), true) => format!("Spring constant: {value} N/m (file, applied)"),
        (Some(value), false) => format!("Spring constant: {value} N/m (file, not applied)"),
        (None, true) => "Force unavailable: missing or invalid spring constant".to_owned(),
        (None, false) => "Spring constant: missing or invalid".to_owned(),
    };
    ui.small(spring);
}

fn summary(ui: &mut Ui, result: &AfmForceFit) {
    let range = result.range();
    crate::typography::headline_label(ui, format!("Empirical fit: {}", result.model.name));
    ui.small(format!(
        "{} vs {} | No modulus",
        quantity_label(&result.y_name, &result.y_unit),
        quantity_label(&result.x_name, &result.x_unit),
    ));
    ui.small(format!(
        "X range: {:.4} to {:.4}{} | {} points",
        range.min,
        range.max,
        unit_suffix(&result.x_unit),
        result.rows.len()
    ));
    if result.converged {
        ui.small("Converged");
    } else {
        ui.colored_label(ui.visuals().warn_fg_color, "Did not converge");
    }
    for p in &result.parameters {
        let value = p.standard_error.map_or_else(
            || format!("{:.5}", p.value),
            |error| super::curve_fit::fmt_val_sigma(p.value, error),
        );
        if result.model.id == LINEAR_ID && p.name == "a" {
            ui.label(format!(
                "Intercept (a): {value}{}",
                unit_suffix(&result.y_unit)
            ));
        } else if result.model.id == LINEAR_ID && p.name == "b" {
            let unit = if result.x_unit.is_empty() {
                "sample"
            } else {
                &result.x_unit
            };
            let response = if result.y_unit.is_empty() {
                "1"
            } else {
                &result.y_unit
            };
            ui.label(format!("Slope (b): {value} {response}/{unit}"));
        } else {
            ui.label(format!("{}: {value}", p.name));
        }
    }
    if let Some(r_squared) = result.r_squared {
        ui.small(format!("R²: {r_squared:.5}"));
    }
    for notice in &result.notices {
        ui.colored_label(ui.visuals().warn_fg_color, notice);
    }
}

fn unit_suffix(unit: &str) -> String {
    if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    }
}

pub(super) fn command(app: &mut PlotiumApp, ui: &mut Ui, choice: Fit, primary: bool) -> bool {
    let id = CommandId::AfmFit(choice);
    let command = commands::describe(app, id);
    let label = if choice == Fit::ToggleOverlay {
        format!(
            "{}  {}",
            if command.checked == Some(true) {
                egui_phosphor::regular::CHECK
            } else {
                " "
            },
            command.label
        )
    } else {
        format!("{}  {}", command.icon.unwrap_or(""), command.label)
    };
    let mut button = egui::Button::new(label);
    if primary {
        button = button.fill(ui.visuals().selection.bg_fill);
    }
    let clicked = ui
        .add_enabled(command.enabled, button)
        .on_disabled_hover_text(command.disabled_reason.unwrap_or_default())
        .clicked();
    if clicked {
        commands::execute_without_clipboard(id, app, ui.ctx());
    }
    clicked
}
