use egui::DragValue;
use plotium_core::state::{
    PlotiumApp, SpectrumArithmeticDialogState, SpectrumArithmeticKey, SpectrumArithmeticOp,
};
use plotium_processing::arithmetic::SpectrumBinaryOp;

pub(crate) fn open_spectrum_arithmetic_dialog(app: &mut PlotiumApp) {
    let targets = app.spectrum_arithmetic_targets();
    let Some(&first) = targets.first() else {
        app.session.status =
            "Spectrum arithmetic needs a 1D spectrum, pattern or plotted table curve.".into();
        return;
    };
    let a = app
        .active_dataset()
        .filter(|di| targets.contains(di))
        .unwrap_or(first);
    let same_kind = |&i: &usize| {
        std::mem::discriminant(&app.doc.datasets[i]) == std::mem::discriminant(&app.doc.datasets[a])
    };
    let b = targets
        .iter()
        .copied()
        .filter(same_kind)
        .find(|&i| i != a)
        .unwrap_or(a);
    app.session.ui.spectrum_arithmetic_dialog = Some(SpectrumArithmeticDialogState {
        a,
        b,
        op: SpectrumArithmeticOp::SubtractDataset,
        k: 1.0,
        constant: 0.0,
        averaged: Vec::new(),
        compat: None,
    });
}

pub(crate) fn spectrum_arithmetic_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    let Some(mut state) = app.session.ui.spectrum_arithmetic_dialog.take() else {
        return;
    };
    let targets = app.spectrum_arithmetic_targets();
    if targets.is_empty() {
        return;
    }
    if !targets.contains(&state.a) {
        state.a = targets[0];
    }
    if !targets.contains(&state.b) {
        state.b = targets[0];
    }

    let mut run = false;
    let mut cancel = false;
    let modal =
        super::modal(ctx, "spectrum_arithmetic_modal", super::ModalKind::Dialog).show(ctx, |ui| {
            crate::ui::result_follow::register_editor(ui);
            ui.heading("Spectrum arithmetic");
            ui.separator();
            ui.label("Combine traces or apply a constant; the result becomes a new dataset.");
            ui.add_space(4.0);
            dataset_combo(app, ui, "Spectrum A", "arith_a", &targets, &mut state.a);
            egui::ComboBox::from_label("Operation")
                .selected_text(state.op.label())
                .show_ui(ui, |ui| {
                    for op in SpectrumArithmeticOp::ALL {
                        ui.selectable_value(&mut state.op, op, op.label());
                    }
                });
            match state.op {
                SpectrumArithmeticOp::AddDataset | SpectrumArithmeticOp::SubtractDataset => {
                    dataset_combo(app, ui, "Spectrum B", "arith_b", &targets, &mut state.b);
                    ui.horizontal(|ui| {
                        ui.label("Coefficient k");
                        ui.add(DragValue::new(&mut state.k).speed(0.01).max_decimals(4));
                    });
                    ui.small("Result = A ± k·B. Tune k to null a solvent line.");
                }
                SpectrumArithmeticOp::MultiplyConstant => {
                    ui.horizontal(|ui| {
                        ui.label("Factor k");
                        ui.add(DragValue::new(&mut state.k).speed(0.01).max_decimals(4));
                    });
                }
                SpectrumArithmeticOp::AddConstant => {
                    ui.horizontal(|ui| {
                        ui.label("Constant c");
                        ui.add(DragValue::new(&mut state.constant).speed(0.1));
                    });
                }
                SpectrumArithmeticOp::Average => {
                    ui.label("Average A with:");
                    average_checklist(app, ui, &targets, &mut state);
                    ui.small(
                        "The result uses A's axis; each point averages the traces covering it.",
                    );
                }
            }
            let compat = operation_compat(app, &mut state);
            match &compat {
                Ok(None) => {}
                Ok(Some(note)) => {
                    ui.colored_label(ui.visuals().warn_fg_color, note);
                }
                Err(reason) => {
                    ui.colored_label(ui.visuals().error_fg_color, reason);
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(compat.is_ok(), egui::Button::new("Create dataset"))
                    .clicked()
                {
                    run = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if run {
        match state.op {
            SpectrumArithmeticOp::AddDataset => {
                app.combine_spectra_datasets(state.a, state.b, SpectrumBinaryOp::Add, state.k)
            }
            SpectrumArithmeticOp::SubtractDataset => {
                app.combine_spectra_datasets(state.a, state.b, SpectrumBinaryOp::Subtract, state.k)
            }
            SpectrumArithmeticOp::MultiplyConstant => {
                app.scale_spectrum_dataset(state.a, state.k, 0.0)
            }
            SpectrumArithmeticOp::AddConstant => {
                app.scale_spectrum_dataset(state.a, 1.0, state.constant)
            }
            SpectrumArithmeticOp::Average => {
                let sources = average_sources(app, &state);
                app.average_trace_datasets(&sources)
            }
        }
        return;
    }
    if !cancel && !modal.should_close() && app.session.ui.spectrum_arithmetic_dialog.is_none() {
        app.session.ui.spectrum_arithmetic_dialog = Some(state);
    }
}

fn operation_compat(
    app: &PlotiumApp,
    state: &mut SpectrumArithmeticDialogState,
) -> Result<Option<String>, String> {
    let key = SpectrumArithmeticKey {
        a: state.a,
        b: state.b,
        op: state.op,
        averaged: state.averaged.clone(),
        revision: app.doc.automation_revision,
    };
    if let Some((cached, compat)) = &state.compat
        && *cached == key
    {
        return compat.clone();
    }
    let compat = match state.op {
        SpectrumArithmeticOp::AddDataset | SpectrumArithmeticOp::SubtractDataset => {
            app.spectrum_arithmetic_compat(state.a, state.b)
        }
        SpectrumArithmeticOp::Average => app.trace_average_compat(&average_sources(app, state)),
        SpectrumArithmeticOp::MultiplyConstant | SpectrumArithmeticOp::AddConstant => {
            app.spectrum_scale_compat(state.a)
        }
    };
    state.compat = Some((key, compat.clone()));
    compat
}

/// A first, then the checked datasets that still exist.
fn average_sources(app: &PlotiumApp, state: &SpectrumArithmeticDialogState) -> Vec<usize> {
    let checked = state
        .averaged
        .iter()
        .filter_map(|&id| app.doc.dataset_index(id))
        .filter(|&index| index != state.a);
    std::iter::once(state.a).chain(checked).collect()
}

fn average_checklist(
    app: &PlotiumApp,
    ui: &mut egui::Ui,
    targets: &[usize],
    state: &mut SpectrumArithmeticDialogState,
) {
    egui::ScrollArea::vertical()
        .id_salt("arith_average")
        .max_height(180.0)
        .show(ui, |ui| {
            for &index in targets.iter().filter(|&&index| index != state.a) {
                let dataset = &app.doc.datasets[index];
                let id = dataset.resource_id();
                let mut checked = state.averaged.contains(&id);
                if ui.checkbox(&mut checked, dataset.display_name()).changed() {
                    if checked {
                        state.averaged.push(id);
                    } else {
                        state.averaged.retain(|&other| other != id);
                    }
                }
            }
        });
}

fn dataset_combo(
    app: &PlotiumApp,
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    targets: &[usize],
    selected: &mut usize,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .width(280.0)
            .selected_text(app.doc.datasets[*selected].display_name())
            .show_ui(ui, |ui| {
                for &di in targets {
                    ui.selectable_value(selected, di, app.doc.datasets[di].display_name());
                }
            });
    });
}
