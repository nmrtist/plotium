//! Plane navigation stays beside the active spectrum, not in a separate workflow.
use egui::{ComboBox, DragValue, Slider, Ui};
use plotium_core::state::{Dataset, PlotiumApp};

use super::commands::{self, CommandId};

pub(super) fn step_gate(app: &PlotiumApp, forward: bool) -> Result<(), &'static str> {
    let volume = app
        .active_dataset()
        .and_then(|di| app.doc.datasets.get(di))
        .and_then(Dataset::as_nmr2d)
        .and_then(|n| n.volume.as_ref())
        .ok_or("Select a three-dimensional NMR spectrum to browse planes.")?;
    if forward && volume.index + 1 >= volume.processed.shape()[volume.fixed_axis] {
        return Err("Choose an earlier plane to move forward.");
    }
    if !forward && volume.index == 0 {
        return Err("Choose a later plane to move backward.");
    }
    Ok(())
}

pub(super) fn step(app: &mut PlotiumApp, forward: bool) {
    let Some(di) = app.active_dataset() else {
        return;
    };
    let Some(volume) = app.doc.datasets[di]
        .as_nmr2d()
        .and_then(|n| n.volume.as_ref())
    else {
        return;
    };
    let axis = volume.fixed_axis;
    let index = if forward {
        volume.index.saturating_add(1)
    } else {
        volume.index.saturating_sub(1)
    };
    select(app, di, axis, index);
}

fn select(app: &mut PlotiumApp, di: usize, axis: usize, index: usize) {
    let id = app.doc.datasets[di].resource_id();
    if let Err(error) = app.select_nmr_volume_plane(id, axis, index) {
        app.session.status = format!("Cannot change NMR plane: {error}");
    }
}

pub(super) fn navigation(app: &mut PlotiumApp, ui: &mut Ui) {
    let Some(di) = app.active_dataset() else {
        return;
    };
    let Some(volume) = app.doc.datasets[di]
        .as_nmr2d()
        .and_then(|n| n.volume.as_ref())
    else {
        return;
    };
    let shape = volume.processed.shape();
    let anchor = volume.anchor;
    let original_axis = volume.fixed_axis;
    let original_index = volume.index;
    let mut axis = original_axis;
    let mut index = original_index;
    let labels: Vec<_> = (0..3)
        .map(|a| {
            let nucleus = volume.source.axes()[a]
                .nucleus
                .as_deref()
                .unwrap_or("unknown");
            format!("{nucleus} (F{})", a + 1)
        })
        .collect();
    let mut ppm = match volume.processed.axis_coordinate(axis, index) {
        Ok(ppm) => ppm,
        Err(error) => {
            ui.colored_label(ui.visuals().error_fg_color, error);
            return;
        }
    };
    let mut ppm_changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.strong("3D plane");
        ComboBox::from_id_salt("nmr_plane_axis")
            .selected_text(format!("Fixed {}", labels[axis]))
            .show_ui(ui, |ui| {
                for (a, label) in labels.iter().enumerate() {
                    ui.selectable_value(&mut axis, a, format!("Fixed {label}"));
                }
            });
        for (command, label) in [
            (CommandId::PreviousNmrPlane, "◀"),
            (CommandId::NextNmrPlane, "▶"),
        ] {
            let description = commands::describe(app, command);
            if ui
                .add_enabled(description.enabled, egui::Button::new(label))
                .on_hover_text(&description.label)
                .on_disabled_hover_text(description.disabled_reason.unwrap_or_default())
                .clicked()
            {
                commands::execute_without_clipboard(command, app, ui.ctx());
            }
        }
        ui.add(Slider::new(&mut index, 0..=shape[original_axis] - 1).show_value(false));
        ui.label(format!("{} / {}", index + 1, shape[original_axis]));
        ppm_changed = ui
            .add(
                DragValue::new(&mut ppm)
                    .speed(0.01)
                    .max_decimals(4)
                    .suffix(" ppm"),
            )
            .changed();
    });
    if axis != original_axis {
        select(app, di, axis, anchor[axis]);
    } else if ppm_changed {
        let volume = app.doc.datasets[di]
            .as_nmr2d()
            .and_then(|n| n.volume.as_ref())
            .expect("local volume invariant");
        match volume.processed.nearest(axis, ppm) {
            Ok(index) => select(app, di, axis, index),
            Err(error) => app.session.status = format!("Cannot locate NMR plane: {error}"),
        }
    } else if index != original_index {
        select(app, di, axis, index);
    }
}

pub(super) fn apply_phase(app: &mut PlotiumApp, ctx: &egui::Context) {
    let Some(di) = app.active_dataset() else {
        return;
    };
    let id = app.doc.datasets[di].resource_id();
    if let Err(reason) = phase_gate(app) {
        app.session.status = reason.into();
        return;
    }
    let draft = app.session.ui.nmr_volume_phase_drafts[&id].clone();
    let Some(mut dataset) = app.doc.datasets[di].as_nmr2d().cloned() else {
        return;
    };
    let expected = dataset
        .volume
        .as_ref()
        .expect("local volume invariant")
        .phases;
    if phase_busy(ctx) {
        return;
    }
    app.session.ui.nmr_volume_phase_busy = true;
    let cancellation = nmr::CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    let output = std::sync::Arc::new(std::sync::Mutex::new(None));
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("volume_phase_job"),
            PhaseJob {
                id,
                expected,
                cancellation,
                output: output.clone(),
            },
        )
    });
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let mut context = nmr::ExecutionContext::default().with_cancellation(worker_cancellation);
        let result = dataset
            .set_volume_phases_with_context(draft.phases, &mut context)
            .map(|()| dataset);
        match output.lock() {
            Ok(mut output) => *output = Some(result),
            Err(error) => {
                *error.into_inner() = Some(Err("NMR phase worker state was interrupted".into()))
            }
        }
        ctx.request_repaint();
    });
}

pub(super) fn processing(app: &mut PlotiumApp, host: &mut Ui, di: usize) {
    use super::tools::task_card;
    use plotium_core::state::TaskDockTab;
    let id = app.doc.datasets[di].resource_id();
    let Some(volume) = app.doc.datasets[di]
        .as_nmr2d()
        .and_then(|n| n.volume.as_ref())
    else {
        return;
    };
    let committed = volume.phases;
    let mut draft = app
        .session
        .ui
        .nmr_volume_phase_drafts
        .get(&id)
        .cloned()
        .filter(|draft| draft.baseline == committed)
        .unwrap_or(plotium_core::state::NmrVolumePhaseDraft {
            baseline: committed,
            phases: committed,
        });
    let geometry = task_card::geometry(
        app,
        host,
        TaskDockTab::Processing,
        300.0,
        app.session.ui.processing_task_collapsed,
    );
    let collapsed = app.session.ui.processing_task_collapsed;
    let area_id = egui::Id::new("processing_task_card");
    let mut close = false;
    task_card::area(host, area_id, geometry.pos).show(host.ctx(), |ui| {
        ui.set_width(geometry.width);
        crate::ui::card_frame(ui.visuals().dark_mode, egui::Margin::ZERO).show(ui, |ui| {
            task_card::header(ui, area_id, "3D Processing", Some("Phase adjustment"), |ui| {
                if crate::ui::panel_chrome::close(ui, "Close Processing").clicked() { close = true; }
            });
            if collapsed { return; }
            egui::ScrollArea::vertical().max_height(geometry.body_height).show(ui, |ui| {
            ui.small("Basic transform completed. Phase is user-adjustable; an absorptive spectrum has not been independently confirmed.");
            for (axis, phase) in draft.phases.iter_mut().enumerate() {
                ui.separator();
                ui.strong(format!("Acquisition F{}", axis + 1));
                ui.horizontal(|ui| {
                    ui.label("Zero order");
                    ui.add(DragValue::new(&mut phase.p0_degrees).speed(0.1).suffix("°"));
                });
                ui.horizontal(|ui| {
                    ui.label("First order");
                    ui.add(DragValue::new(&mut phase.p1_degrees).speed(0.1).suffix("°"));
                });
                ui.horizontal(|ui| {
                    ui.label("Pivot fraction");
                    ui.add(DragValue::new(&mut phase.pivot_fraction).range(0.0..=1.0).speed(0.001));
                });
            }
            app.session.ui.nmr_volume_phase_drafts.insert(id, draft.clone());
            let cancel = commands::describe(app, CommandId::CancelNmrVolumePhase);
            if ui.add_enabled(cancel.enabled, egui::Button::new("Cancel processing")).on_disabled_hover_text(cancel.disabled_reason.unwrap_or_default()).clicked() {
                commands::execute_without_clipboard(CommandId::CancelNmrVolumePhase, app, ui.ctx());
            }
            if app.session.ui.nmr_volume_phase_busy { ui.spinner(); ui.label("Processing phase…"); }
            let desc = commands::describe(app, CommandId::ApplyNmrVolumePhase);
            if ui.add_enabled(desc.enabled, egui::Button::new("Apply phase"))
                .on_disabled_hover_text(desc.disabled_reason.unwrap_or_default()).clicked() {
                commands::execute_without_clipboard(CommandId::ApplyNmrVolumePhase, app, ui.ctx());
            }
            });
        });
    });
    if close {
        app.session.ui.close_task_tab(TaskDockTab::Processing);
    }
}

#[derive(Clone)]
struct PhaseJob {
    id: plotium_core::state::DatasetId,
    expected: [plotium_processing::nmr_volume::VolumePhase; 3],
    cancellation: nmr::CancellationToken,
    output:
        std::sync::Arc<std::sync::Mutex<Option<Result<plotium_core::state::Nmr2DDataset, String>>>>,
}
fn phase_busy(ctx: &egui::Context) -> bool {
    ctx.data(|data| {
        data.get_temp::<PhaseJob>(egui::Id::new("volume_phase_job"))
            .is_some()
    })
}
pub(super) fn poll_phase(app: &mut PlotiumApp, ctx: &egui::Context) {
    let key = egui::Id::new("volume_phase_job");
    let Some(job) = ctx.data(|data| data.get_temp::<PhaseJob>(key)) else {
        return;
    };
    let result = match job.output.lock() {
        Ok(mut output) => output.take(),
        Err(error) => Some(Err(format!("NMR phase worker failed: {error}"))),
    };
    let Some(result) = result else { return };
    ctx.data_mut(|data| data.remove::<PhaseJob>(key));
    app.session.ui.nmr_volume_phase_busy = false;
    if job.cancellation.is_cancelled() {
        app.session.status = "NMR phase processing cancelled".into();
        return;
    }
    let result =
        result.and_then(|dataset| app.install_nmr_volume_result(job.id, job.expected, dataset));
    if let Err(error) = result {
        app.session.status = format!("Cannot apply NMR phase: {error}");
    }
}

pub(super) fn cancel_phase(ctx: &egui::Context) {
    if let Some(job) = ctx.data(|data| data.get_temp::<PhaseJob>(egui::Id::new("volume_phase_job")))
    {
        job.cancellation.cancel();
    }
}

pub(super) fn phase_gate(app: &PlotiumApp) -> Result<(), &'static str> {
    if app.session.ui.nmr_volume_phase_busy {
        return Err(
            "Wait for phase processing to finish or cancel it before applying new settings.",
        );
    }
    if !app
        .active_dataset()
        .and_then(|di| app.doc.datasets.get(di))
        .and_then(Dataset::as_nmr2d)
        .is_some_and(|n| n.volume.is_some())
    {
        return Err("Select a three-dimensional NMR spectrum before adjusting its phase.");
    }
    let data = &app.doc.datasets[app.active_dataset().expect("validated active dataset")];
    let phases = data
        .as_nmr2d()
        .and_then(|n| n.volume.as_ref())
        .expect("validated volume")
        .phases;
    if !app
        .session
        .ui
        .nmr_volume_phase_drafts
        .get(&data.resource_id())
        .is_some_and(|draft| draft.baseline == phases && draft.phases != phases)
    {
        return Err("Open 3D Processing and edit a phase setting before applying phase.");
    }
    Ok(())
}

pub(super) fn editing_gate(app: &PlotiumApp, id: CommandId) -> Result<(), &'static str> {
    use plotium_core::state::Tool;
    let volume = app
        .active_dataset()
        .and_then(|di| app.doc.datasets.get(di))
        .and_then(Dataset::as_nmr2d)
        .is_some_and(|n| n.volume.is_some());
    if volume
        && matches!(
            id,
            CommandId::DetectPeaks
                | CommandId::PeakList
                | CommandId::Integrate
                | CommandId::Tool(
                    Tool::Peaks | Tool::Integrate | Tool::Symmetry | Tool::ManualPhase
                )
        )
    {
        return Err(
            "Open a standalone spectrum for peak or integral editing; use 3D Processing to adjust volume phase.",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_navigation_renders_and_disables_endpoint_steps() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/bruker-3d-qf");
        let loaded = plotium_io::load_path(&path).unwrap();
        let (dataset, _) =
            plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
        let mut app = PlotiumApp::new();
        app.doc.datasets.push(dataset);
        app.focus_single(0);
        let id = app.doc.datasets[0].resource_id();
        app.select_nmr_volume_plane(id, 0, 0).unwrap();
        assert!(!commands::describe(&app, CommandId::PreviousNmrPlane).enabled);
        assert!(commands::describe(&app, CommandId::NextNmrPlane).enabled);
        let description = commands::describe(&app, CommandId::ApplyNmrVolumePhase);
        assert!(!description.enabled);
        assert!(
            description
                .disabled_reason
                .unwrap()
                .contains("Open 3D Processing")
        );
        let committed = app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .phases;
        let mut draft = plotium_core::state::NmrVolumePhaseDraft {
            baseline: committed,
            phases: committed,
        };
        app.session
            .ui
            .nmr_volume_phase_drafts
            .insert(id, draft.clone());
        assert!(!commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
        draft.phases[0].p0_degrees = 15.0;
        app.session.ui.nmr_volume_phase_drafts.insert(id, draft);
        assert!(commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
        // Readiness belongs to the selected dataset, not the last opened card.
        app.doc.datasets.push({
            let loaded = plotium_io::load_path(&path).unwrap();
            plotium_core::workflow::dataset_from_acquisition(loaded.acquisition)
                .unwrap()
                .0
        });
        app.focus_single(1);
        assert!(!commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
        app.focus_single(0);
        app.session
            .ui
            .nmr_volume_phase_drafts
            .get_mut(&id)
            .unwrap()
            .baseline[0]
            .p0_degrees = 90.0;
        assert!(!commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(Default::default(), |ui| navigation(&mut app, ui));
        step(&mut app, true);
        assert_eq!(
            app.doc.datasets[0]
                .as_nmr2d()
                .unwrap()
                .volume
                .as_ref()
                .unwrap()
                .index,
            1
        );
    }

    #[test]
    fn plane_commands_explain_missing_volume_and_phase_cancel_tracks_worker() {
        let mut app = PlotiumApp::new();
        for id in [
            CommandId::PreviousNmrPlane,
            CommandId::NextNmrPlane,
            CommandId::ApplyNmrVolumePhase,
        ] {
            let description = commands::describe(&app, id);
            assert!(!description.enabled);
            assert!(description.disabled_reason.unwrap().contains("Select"));
        }
        assert!(!commands::describe(&app, CommandId::CancelNmrVolumePhase).enabled);
        app.session.ui.nmr_volume_phase_busy = true;
        assert!(commands::describe(&app, CommandId::CancelNmrVolumePhase).enabled);
    }
}
