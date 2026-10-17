use super::task_card;
use crate::ui::commands::{self, CommandId};
use egui::Ui;
use plotium_core::{
    DiffusionInput, DiffusionModel,
    state::{PlotiumApp, TaskDockTab},
};

#[derive(Clone, Default)]
struct Draft {
    direct: bool,
    bpp: bool,
    values: String,
    unit: usize,
    gamma: String,
    delta: String,
    big_delta: String,
    tau_1: String,
    tau_2: String,
    error: Option<String>,
}

pub(crate) fn open_task(app: &mut PlotiumApp, di: usize) {
    let Some(data) = app.doc.datasets.get(di).and_then(|data| data.as_nmr2d()) else {
        return;
    };
    app.session.ui.dosy_task_dataset = Some(data.resource_id);
    app.session.ui.open_task_tab(TaskDockTab::Dosy);
}

pub(crate) fn render_task(app: &mut PlotiumApp, host: &mut Ui) {
    if !task_card::is_active(app, TaskDockTab::Dosy) {
        return;
    }
    let Some(di) = app
        .session
        .ui
        .dosy_task_dataset
        .and_then(|id| app.doc.dataset_index(id))
    else {
        return;
    };
    if app.active_dataset() != Some(di) {
        return;
    }
    let card = task_card::geometry(app, host, TaskDockTab::Dosy, 300.0, false);
    let id = egui::Id::new("dosy_task_card");
    let mut close = false;
    task_card::area(host, id, card.pos).show(host.ctx(), |ui| {
        ui.set_width(card.width);
        crate::ui::card_frame(ui.visuals().dark_mode, egui::Margin::ZERO).show(ui, |ui| {
            task_card::header(ui, id, "DOSY", None::<String>, |ui| {
                close = crate::ui::panel_chrome::close(ui, "Close DOSY").clicked();
            });
            task_card::tab_bar(app, TaskDockTab::Dosy, ui);
            task_card::sized_body(ui, card.body_height, |ui| body(app, di, ui));
        });
        task_card::resize_handles(app, ui, id, TaskDockTab::Dosy, card.width, card.body_height);
    });
    if close {
        app.session.ui.close_task_tab(TaskDockTab::Dosy);
    }
}

fn initial_draft(app: &PlotiumApp, di: usize) -> Draft {
    let Some(data) = app.doc.datasets[di].as_nmr2d() else {
        return Draft::default();
    };
    let input = data
        .resolve_diffusion_input(None)
        .ok()
        .or_else(|| data.suggested_diffusion_input().ok());
    let mut draft = Draft {
        gamma: plotium_io::gyromagnetic_ratio(&data.data.direct.nucleus)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        ..Draft::default()
    };
    if let Some(input) = input {
        match input.model {
            DiffusionModel::DirectB { .. } => {
                draft.direct = true;
                draft.values = input
                    .increments
                    .iter()
                    .map(|e| e.b.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            DiffusionModel::JeolBppDste {
                gamma,
                delta,
                big_delta,
                tau_1,
                tau_2,
            } => {
                draft.bpp = true;
                draft.gamma = gamma.to_string();
                draft.delta = delta.to_string();
                draft.big_delta = big_delta.to_string();
                draft.tau_1 = tau_1.to_string();
                draft.tau_2 = tau_2.to_string();
            }
            DiffusionModel::RectangularPgse {
                gamma,
                delta,
                big_delta,
                tau,
            }
            | DiffusionModel::StejskalTanner {
                gamma,
                delta,
                big_delta,
                tau,
                ..
            } => {
                draft.gamma = gamma.to_string();
                draft.delta = delta.to_string();
                draft.big_delta = big_delta.to_string();
                draft.tau_1 = tau.to_string();
            }
        }
    }
    draft
}

fn body(app: &mut PlotiumApp, di: usize, ui: &mut Ui) {
    let data = app.doc.datasets[di].as_nmr2d().unwrap();
    let draft_id = egui::Id::new(("dosy_input_draft", data.resource_id));
    let mut draft = ui
        .ctx()
        .data(|data| data.get_temp::<Draft>(draft_id))
        .unwrap_or_else(|| initial_draft(app, di));
    match data.resolve_diffusion_input(None) {
        Ok(input) => {
            ui.small(&input.source);
            let b = input.b_values(data.data.rows);
            if let Ok(b) = b {
                let lo = b.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = b.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                ui.label(format!(
                    "{} increments · b = {lo:.6e}…{hi:.6e} s/m²",
                    b.len()
                ));
            }
            if let Some(c) = input.vendor_coefficient_seconds {
                ui.small(format!("Vendor st_coef: {:.7} ms", c * 1e3));
            }
        }
        Err(error) => {
            ui.label(error);
        }
    }
    if let Some(note) = &data.dosy_provenance_warning {
        ui.colored_label(ui.visuals().warn_fg_color, note);
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.selectable_value(&mut draft.direct, false, "Sequence model");
        ui.selectable_value(&mut draft.direct, true, "Direct b values");
    });
    if draft.direct {
        ui.small("One value per acquired increment, in acquisition order. Paste numbers separated by spaces, commas or newlines.");
        egui::ComboBox::from_label("b unit")
            .selected_text(["s/m²", "s/cm²", "s/mm²"][draft.unit])
            .show_ui(ui, |ui| {
                for (i, unit) in ["s/m²", "s/cm²", "s/mm²"].iter().enumerate() {
                    ui.selectable_value(&mut draft.unit, i, *unit);
                }
            });
        ui.add(egui::TextEdit::multiline(&mut draft.values).desired_rows(5));
        if ui.button("Import b values…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Text", &["txt", "csv", "tsv"])
                .pick_file()
        {
            match std::fs::read_to_string(path) {
                Ok(text) => draft.values = text,
                Err(error) => draft.error = Some(error.to_string()),
            }
        }
    } else {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut draft.bpp, false, "Rectangular PGSE / STE");
            ui.selectable_value(&mut draft.bpp, true, "JEOL BPP-DSTE");
        });
        ui.small("Rectangular gradients only. Durations in seconds; γ in rad/(s·T). Empty values remain missing. Confirm the actual sequence before applying.");
        for (label, value) in [
            ("γ", &mut draft.gamma),
            ("δ", &mut draft.delta),
            ("Δ", &mut draft.big_delta),
            ("τ / τ₁", &mut draft.tau_1),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.text_edit_singleline(value);
            });
        }
        if draft.bpp {
            ui.horizontal(|ui| {
                ui.label("τ₂");
                ui.text_edit_singleline(&mut draft.tau_2);
            });
        }
    }
    if ui.button("Confirm and apply analysis input").clicked() {
        match parse_draft(&draft, app, di) {
            Ok(input) => {
                app.set_diffusion_input(di, Some(input));
                draft.error = None;
            }
            Err(error) => draft.error = Some(error),
        }
    }
    if ui.button("Use acquisition suggestion").clicked() {
        app.set_diffusion_input(di, None);
        ui.ctx().data_mut(|data| data.remove::<Draft>(draft_id));
        draft = initial_draft(app, di);
    }
    if let Some(error) = &draft.error {
        ui.colored_label(ui.visuals().warn_fg_color, error);
    }
    ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
    ui.separator();
    let mut params = app.resolve_ilt_params_for(di, app.explicit_ilt_input_for(di));
    let before = params;
    ui.collapsing("ILT settings", |ui| {
        for (label, value, speed) in [
            ("λ", &mut params.lambda, 0.001),
            ("D min (m²/s)", &mut params.d_min, 1e-12),
            ("D max (m²/s)", &mut params.d_max, 1e-10),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(value).speed(speed));
            });
        }
        ui.add(
            egui::DragValue::new(&mut params.n_grid)
                .range(16..=512)
                .prefix("Grid points "),
        );
    });
    if params != before {
        app.set_explicit_ilt_input(di, params);
    }
    ui.horizontal(|ui| {
        ui.label("Show");
        for (label, mode) in [
            ("Stack", plotium_core::PseudoDisplay::Stack),
            ("DOSY map", plotium_core::PseudoDisplay::DosyMap),
        ] {
            if ui
                .selectable_label(
                    app.doc.datasets[di]
                        .as_nmr2d()
                        .is_some_and(|data| data.display == mode),
                    label,
                )
                .clicked()
            {
                app.set_pseudo_display(di, mode);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Map method");
        let data = app.doc.datasets[di].as_nmr2d().unwrap();
        let selected = data.dosy_method;
        let mono_available = data.dosy_map.is_some();
        let ilt_available = data.ilt_map.is_some();
        let ilt_params = app.resolve_ilt_params_for(di, None);
        for (label, method, available) in [
            (
                "Per-column",
                plotium_core::DosyMethod::MonoExp,
                mono_available,
            ),
            (
                "ILT",
                plotium_core::DosyMethod::Ilt(ilt_params),
                ilt_available,
            ),
        ] {
            if ui
                .add_enabled(
                    available,
                    egui::Button::selectable(selected == method, label),
                )
                .on_disabled_hover_text("Build this map to view its result.")
                .clicked()
            {
                app.set_pseudo_dosy_method(di, method);
            }
        }
    });
    // Always show both builders; the command catalog explains missing inputs.
    for id in [
        CommandId::BuildDosy,
        CommandId::BuildIlt,
        CommandId::CancelDosy,
    ] {
        let command = commands::describe(app, id);
        if ui
            .add_enabled(command.enabled, egui::Button::new(&command.label))
            .on_disabled_hover_text(command.disabled_reason.unwrap_or(""))
            .clicked()
        {
            commands::execute_without_clipboard(id, app, ui.ctx());
        }
    }
}

fn parse_draft(draft: &Draft, app: &PlotiumApp, di: usize) -> Result<DiffusionInput, String> {
    let data = app.doc.datasets[di]
        .as_nmr2d()
        .ok_or("Select an NMR stack.")?;
    let input = if draft.direct {
        let values = draft
            .values
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<f64>()
                    .map_err(|_| format!("Invalid b value: {s}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        DiffusionInput::direct(&values, ["s/m²", "s/cm²", "s/mm²"][draft.unit])?
    } else {
        let number = |text: &str, name: &str| {
            text.parse::<f64>()
                .map_err(|_| format!("Enter {name}; missing values are not assumed to be zero."))
        };
        let gamma = number(&draft.gamma, "γ")?;
        let delta = number(&draft.delta, "δ")?;
        let big_delta = number(&draft.big_delta, "Δ")?;
        let tau = number(&draft.tau_1, "τ / τ₁")?;
        let model = if draft.bpp {
            DiffusionModel::JeolBppDste {
                gamma,
                delta,
                big_delta,
                tau_1: tau,
                tau_2: number(&draft.tau_2, "τ₂")?,
            }
        } else {
            DiffusionModel::RectangularPgse {
                gamma,
                delta,
                big_delta,
                tau,
            }
        };
        let axis = data
            .data
            .pseudo_axis
            .as_ref()
            .filter(|axis| axis.kind == plotium_io::PseudoKind::Gradient && axis.unit == "T/m")
            .ok_or("Provide a gradient axis in T/m or use direct b values.")?;
        DiffusionInput::from_model(model, &axis.values)?
    };
    input.b_values(data.data.rows)?;
    Ok(input)
}
