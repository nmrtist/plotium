use egui::{DragValue, Ui};
use plotium_core::actions::{Action, DatasetProcessingState};
use plotium_core::state::PlotiumApp;
use plotium_processing::{Layout2D, Preset2D};

pub(super) fn experiment_group(app: &mut PlotiumApp, di: usize, ui: &mut Ui) -> bool {
    let n = app.doc.datasets[di].as_nmr2d().unwrap();
    let mut chosen = n.preset;
    egui::ComboBox::from_label("Experiment")
        .selected_text(chosen.label())
        .show_ui(ui, |ui| {
            for &p in Preset2D::all() {
                ui.selectable_value(&mut chosen, p, p.label());
            }
        });
    if chosen != n.preset {
        let before = DatasetProcessingState::from_dataset(&app.doc.datasets[di]);
        let mut after = before.clone();
        if let DatasetProcessingState::Nmr2D { params, preset, .. } = &mut after {
            *preset = chosen;
            params.layout = chosen.layout();
        }
        app.execute_action(Action::update_dataset_processing(
            app.doc.datasets[di].resource_id(),
            before,
            after,
        ));
    }

    let is_stack = {
        let n = app.doc.datasets[di].as_nmr2d().unwrap();
        let layout = match n.params.layout {
            Layout2D::Ft => "Contour (true 2D FT)",
            Layout2D::Stack
                if n.native_processed.dataset().as_raw().is_some() && n.data.nus.is_some() =>
            {
                "Acquired NUS observations (not reconstructed)"
            }
            Layout2D::Stack if n.data.nus.is_some() => "Stack (reconstructed NUS slices)",
            Layout2D::Stack => "Stack (pseudo-2D 1D slices)",
        };
        ui.label(format!("Layout: {layout}"));
        matches!(n.params.layout, Layout2D::Stack)
    };

    super::cursor_group(app, di, ui);
    nus_group(app, di, ui);

    if is_stack {
        ui.separator();
        pseudo_group(app, di, ui);
    }

    if !is_stack {
        super::symmetry_group(app, di, ui);
        super::integrate_group(app, di, ui);
    }

    super::slice_group(app, di, ui);
    false
}

/// Reconstruction inputs apply to the imported sampling coordinates.
fn nus_group(app: &mut PlotiumApp, di: usize, ui: &mut Ui) {
    let Some(dataset) = app.doc.datasets[di].as_nmr2d() else {
        return;
    };
    let Some(nus) = &dataset.data.nus else {
        return;
    };
    ui.separator();
    ui.label(crate::typography::headline("Non-uniform sampling"));
    if let Some(warning) = &dataset.reconstruction_warning {
        ui.colored_label(ui.visuals().warn_fg_color, warning);
    }
    ui.small(format!(
        "{} observations on a {}-point indirect grid.",
        nus.acquired, nus.grid
    ));
    let mut request = dataset.nus_request.unwrap_or_default();
    let mut noise_known = request.noise_standard_deviation.is_some();
    let mut changed = ui
        .checkbox(&mut noise_known, "Override automatic noise estimate")
        .changed();
    let mut noise = request.noise_standard_deviation.unwrap_or(0.0);
    changed |= ui
        .add_enabled(
            noise_known,
            DragValue::new(&mut noise)
                .range(0.0..=f64::MAX)
                .prefix("Noise σ "),
        )
        .changed();
    ui.small("Noise is estimated automatically from acquired data. An override uses the standard deviation after the current F2 recipe; zero explicitly asserts noiseless input.");
    request.noise_standard_deviation = noise_known.then_some(noise);
    changed |= ui
        .add(
            DragValue::new(&mut request.max_iterations)
                .range(1..=2048)
                .prefix("Maximum iterations "),
        )
        .changed();
    let before = DatasetProcessingState::from_dataset(&app.doc.datasets[di]);
    let mut after = before.clone();
    if let DatasetProcessingState::Nmr2D { nus_request, .. } = &mut after {
        *nus_request = Some(request);
    }
    if changed && before != after {
        app.execute_action(Action::update_dataset_processing(
            app.doc.datasets[di].resource_id(),
            before,
            after,
        ));
    }
    ui.small("NUS reconstruction runs automatically with the F2 FFT. The F1 FFT produces the second frequency axis.");
}

fn pseudo_group(app: &mut PlotiumApp, di: usize, ui: &mut Ui) {
    if let Some(axis) = app.doc.datasets[di]
        .as_nmr2d()
        .and_then(|n| n.data.pseudo_axis.as_ref())
    {
        ui.small(format!(
            "Array: {} — {} points in {}",
            axis.name,
            axis.values.len(),
            axis.unit
        ));
    }
    if ui.button("DOSY…").clicked() {
        crate::ui::commands::execute_without_clipboard(
            crate::ui::commands::CommandId::Dosy,
            app,
            ui.ctx(),
        );
    }
}
