//! XAS processing card: the signal, normalization, background and transform
//! parameters behind a scan's curves. Every value is applied as one Undo step.

use super::*;
use crate::ui::tools::task_card::{self, TaskCardGeometry};
use egui::RichText;
use plotium_core::state::{TaskDockTab, XasCalibrationError, XasDataset, XasSignal};
use plotium_io::xas::XasColumnId;
use plotium_processing::xas::{XafsWindow, XasPreset, XasProcessing};

const WINDOWS: [(XafsWindow, &str); 3] = [
    (XafsWindow::KaiserBessel, "Kaiser-Bessel"),
    (XafsWindow::Hanning, "Hanning"),
    (XafsWindow::Welch, "Welch"),
];

pub(super) fn render(app: &mut PlotiumApp, host: &mut Ui, di: usize) {
    let Some(dataset) = app.doc.datasets[di].as_xas() else {
        return;
    };
    let before = DatasetProcessingState::from_dataset(&app.doc.datasets[di]);
    let snapshot = dataset.clone();
    let mut signal = dataset.signal;
    let mut params = dataset.params;
    let name = app.doc.datasets[di].display_name();
    let TaskCardGeometry {
        pos,
        width,
        body_height,
    } = task_card::geometry(
        app,
        host,
        TaskDockTab::Processing,
        520.0,
        app.session.ui.processing_task_collapsed,
    );
    let collapsed = app.session.ui.processing_task_collapsed;
    let dark = host.visuals().dark_mode;
    let (mut close, mut toggle, mut changed) = (false, false, false);

    let area_id = egui::Id::new("processing_task_card");
    task_card::area(host, area_id, pos).show(host.ctx(), |ui| {
        ui.set_width(width);
        crate::ui::card_frame(dark, egui::Margin::ZERO).show(ui, |ui| {
            task_card::header(ui, area_id, "XAS Processing", None::<&str>, |ui| {
                if crate::ui::panel_chrome::close(ui, "Close Processing").clicked() {
                    close = true;
                }
                if crate::ui::panel_chrome::collapse(ui, collapsed, "Processing").clicked() {
                    toggle = true;
                }
                ui.menu_button(icon::DOTS_THREE_VERTICAL, |ui| panel_menu(app, di, ui));
            });
            if task_card::tab_bar(app, TaskDockTab::Processing, ui) {
                ui.separator();
            }
            ui.add(egui::Label::new(RichText::new(name).small()).truncate());
            if !collapsed {
                ui.separator();
                task_card::sized_body(ui, body_height, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            changed |= body(ui, &snapshot, &mut signal, &mut params);
                        });
                });
            }
        });
        if !collapsed {
            task_card::resize_handles(
                app,
                ui,
                area_id,
                TaskDockTab::Processing,
                width,
                body_height,
            );
        }
    });
    // A drag edits on every frame; the session makes it a single Undo step.
    let dragging = host.ctx().dragged_id().is_some();
    if changed {
        if dragging {
            app.begin_processing_session(di);
        }
        app.commit_processing_edit(
            di,
            before,
            DatasetProcessingState::Xas {
                signal,
                processing: Box::new(params),
            },
        );
    }
    if !dragging {
        app.finish_processing_session();
    }
    if toggle {
        app.session.ui.processing_task_collapsed = !collapsed;
    }
    if close {
        app.session.ui.close_task_tab(TaskDockTab::Processing);
    }
}

fn body(
    ui: &mut Ui,
    dataset: &XasDataset,
    signal: &mut Option<XasSignal>,
    params: &mut XasProcessing,
) -> bool {
    let mut changed = false;
    for issue in dataset.issues() {
        ui.label(
            RichText::new(issue)
                .small()
                .color(ui.visuals().warn_fg_color),
        );
    }
    if let Some(edge) = &dataset.processed.edge {
        ui.label(
            RichText::new(format!(
                "E0 {:.2} eV · edge step {:.4}",
                edge.e0, edge.edge_step
            ))
            .small(),
        );
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Preset");
        for preset in XasPreset::ALL {
            if ui.small_button(preset.label()).clicked() {
                *params = preset.apply(params, dataset.k_end());
                changed = true;
            }
        }
    });
    ui.separator();
    ui.label(RichText::new("Signal").strong());
    changed |= signal_controls(ui, dataset, signal);
    ui.separator();
    ui.label(RichText::new("Energy").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("Shift (eV)");
        let mut shift = params.energy_shift;
        if number(ui, &mut shift, 0.1) {
            *params = params.with_energy_shift(shift);
            changed = true;
        }
        match dataset.calibration() {
            Ok(target) => {
                let label = format!(
                    "Set {} {} to {:.1} eV",
                    target.element, target.edge, target.energy
                );
                if ui
                    .small_button(label)
                    .on_hover_text("Shifts the reference foil's edge to its tabulated energy.")
                    .clicked()
                {
                    *params = params.with_energy_shift(target.shift);
                    changed = true;
                }
            }
            Err(XasCalibrationError::NoReference) => {
                ui.weak("No reference channel to calibrate with.");
            }
            Err(XasCalibrationError::NoReferenceEdge) => {
                ui.weak("The reference channel shows no edge to calibrate with.");
            }
            Err(XasCalibrationError::CoarseReference(step)) => {
                ui.weak(format!(
                    "The reference steps {step:.1} eV at its edge, too coarse to calibrate."
                ));
            }
        }
    });
    changed |= optional(ui, "E0 (eV)", &mut params.pre_edge.e0, dataset.edge_e0());
    ui.separator();
    ui.label(RichText::new("Normalization").strong());
    let edge = dataset.processed.edge.as_ref();
    changed |= optional(
        ui,
        "Pre-edge from",
        &mut params.pre_edge.pre1,
        edge.map(|e| e.pre1),
    );
    changed |= optional(
        ui,
        "Pre-edge to",
        &mut params.pre_edge.pre2,
        edge.map(|e| e.pre2),
    );
    changed |= optional(
        ui,
        "Post-edge from",
        &mut params.pre_edge.norm1,
        edge.map(|e| e.norm1),
    );
    changed |= optional(
        ui,
        "Post-edge to",
        &mut params.pre_edge.norm2,
        edge.map(|e| e.norm2),
    );
    ui.separator();
    ui.label(RichText::new("Background").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("Rbkg (Å)");
        changed |= ui
            .add(
                egui::DragValue::new(&mut params.autobk.rbkg)
                    .speed(0.05)
                    .range(0.1..=5.0),
            )
            .changed();
        ui.label("k weight");
        changed |= ui
            .add(egui::DragValue::new(&mut params.autobk.kweight).range(0..=3))
            .changed();
    });
    ui.separator();
    ui.label(RichText::new("Fourier transform").strong());
    let transform = &mut params.transform;
    ui.horizontal_wrapped(|ui| {
        ui.label("k from");
        changed |= number(ui, &mut transform.kmin, 0.1);
        ui.label("to");
        changed |= number(ui, &mut transform.kmax, 0.1);
        ui.label("1/Å");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("k weight");
        changed |= ui
            .add(egui::DragValue::new(&mut transform.kweight).range(0..=3))
            .changed();
        ui.label("Window");
        let selected = WINDOWS
            .iter()
            .find(|(window, _)| *window == transform.window)
            .map_or("Other", |(_, label)| label);
        egui::ComboBox::from_id_salt("xas_window")
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (window, label) in WINDOWS {
                    changed |= ui
                        .selectable_value(&mut transform.window, window, label)
                        .changed();
                }
            });
    });
    changed
}

fn number(ui: &mut Ui, value: &mut f64, speed: f64) -> bool {
    ui.add(egui::DragValue::new(value).speed(speed).max_decimals(3))
        .changed()
}

/// A value Plotium chooses unless the box is ticked. Unticked it shows the
/// value in use, so ticking starts from there.
fn optional(ui: &mut Ui, label: &str, value: &mut Option<f64>, automatic: Option<f64>) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        let mut manual = value.is_some();
        if ui.checkbox(&mut manual, label).changed() {
            *value = manual.then(|| automatic.unwrap_or(0.0));
            changed = true;
        }
        match value {
            Some(value) => changed |= number(ui, value, 0.5),
            None => {
                ui.weak(automatic.map_or("auto".to_owned(), |v| format!("{v:.2}")));
            }
        }
    });
    changed
}

fn signal_controls(ui: &mut Ui, dataset: &XasDataset, signal: &mut Option<XasSignal>) -> bool {
    let mut changed = false;
    let channels = &dataset.scan.channels;
    let others: Vec<XasColumnId> = (0..dataset.scan.columns.len() as u32)
        .map(XasColumnId)
        .filter(|id| *id != channels.energy)
        .collect();
    let Some(&first) = others.first() else {
        ui.weak("The scan has no signal column.");
        return false;
    };
    let transmission = channels
        .i0
        .zip(channels.transmission)
        .map(|(i0, transmitted)| XasSignal::Transmission { i0, transmitted });
    let fluorescence = channels
        .i0
        .zip(channels.fluorescence.first().copied())
        .map(|(i0, signal)| XasSignal::Fluorescence { signal, i0 });
    // A file with no recognized signal still needs a way to name one by hand.
    let absorption = channels
        .mu
        .or(signal.is_none().then_some(first))
        .map(|column| XasSignal::Absorption { column });
    let options = [
        ("Transmission", "I0 and It", transmission),
        ("Fluorescence", "I0 and a fluorescence signal", fluorescence),
        ("Absorption", "an absorption column", absorption),
    ];
    let active = *signal;
    let current = |kind: &str| {
        matches!(
            (active, kind),
            (Some(XasSignal::Transmission { .. }), "Transmission")
                | (Some(XasSignal::Fluorescence { .. }), "Fluorescence")
                | (Some(XasSignal::Absorption { .. }), "Absorption")
        )
    };
    ui.horizontal_wrapped(|ui| {
        for (label, needs, option) in options {
            let selected = current(label);
            let response = ui
                .add_enabled(
                    option.is_some() || selected,
                    egui::Button::selectable(selected, label),
                )
                .on_disabled_hover_text(format!("The file names no {needs}."));
            if response.clicked() && !selected {
                *signal = option;
                changed = true;
            }
        }
    });
    let name = |id: XasColumnId| {
        dataset
            .scan
            .columns
            .get(id.0 as usize)
            .map_or("?", |column| column.name.as_str())
    };
    let mut column = |ui: &mut Ui, label: &str, value: &mut XasColumnId| {
        egui::ComboBox::from_label(label)
            .selected_text(name(*value))
            .show_ui(ui, |ui| {
                for id in &others {
                    changed |= ui.selectable_value(value, *id, name(*id)).changed();
                }
            });
    };
    match signal {
        Some(XasSignal::Transmission { i0, transmitted }) => {
            column(ui, "I0", i0);
            column(ui, "It", transmitted);
        }
        Some(XasSignal::Fluorescence { signal, i0 }) => {
            column(ui, "If", signal);
            column(ui, "I0", i0);
        }
        Some(XasSignal::Absorption { column: id }) => column(ui, "μ", id),
        None => {
            ui.weak("Choose how this scan was measured.");
        }
    }
    changed
}
