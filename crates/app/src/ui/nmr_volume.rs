//! Plane navigation stays beside the active spectrum, not in a separate workflow.
use egui::{ComboBox, DragValue, Slider, Ui};
use plotium_core::state::{Dataset, PlotiumApp};

use super::commands::{self, CommandId};

fn canvas_phase_target(app: &PlotiumApp) -> Option<plotium_core::state::DatasetId> {
    let canvas = app.doc.canvases.get(app.session.active_canvas?)?;
    let plot = canvas
        .selected_plot_object_id()
        .or_else(|| canvas.active_plot_object_id())?;
    canvas.object(plot)?.dataset()
}

pub(super) fn phase_target(app: &PlotiumApp) -> Option<plotium_core::state::DatasetId> {
    use plotium_core::state::SelectionScope;
    let listed = app
        .active_dataset()
        .and_then(|di| app.doc.datasets.get(di))
        .map(Dataset::resource_id);
    match app.session.ui.selection_scope {
        SelectionScope::DataList => listed,
        _ => canvas_phase_target(app).or(listed),
    }
}

pub(super) fn active_volume(app: &PlotiumApp) -> bool {
    phase_target(app)
        .and_then(|id| app.doc.dataset_index(id))
        .and_then(|di| app.doc.datasets.get(di))
        .and_then(Dataset::as_nmr2d)
        .is_some_and(|n| n.volume.is_some())
}

pub(super) fn open_phase(app: &mut PlotiumApp) {
    if let Some(di) = phase_target(app).and_then(|id| app.doc.dataset_index(id)) {
        app.focus_single(di);
    }
    app.cancel_interaction();
    app.set_tool(plotium_core::state::Tool::BrowseZoom);
    super::tools::expand_processing_surface(app);
    app.session.ui.processing_task_collapsed = false;
}

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
    poll_phase(app, ctx);
    if let Err(reason) = phase_gate(app) {
        app.session.status = reason.to_owned();
        return;
    }
    let id = phase_target(app).expect("validated phase target");
    let di = app.doc.dataset_index(id).expect("validated phase dataset");
    let draft = app.session.ui.nmr_volume_phase_drafts[&id].clone();
    let Some(mut dataset) = app.doc.datasets[di].as_nmr2d().cloned() else {
        return;
    };
    let expected = dataset
        .volume
        .as_ref()
        .expect("local volume invariant")
        .phases;
    app.session.ui.nmr_volume_phase_busy = true;
    let name = app.doc.datasets[di].display_name();
    app.session.status = format!("Calculating phase for {name}…");
    let cancellation = nmr::CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    let output = std::sync::Arc::new(std::sync::Mutex::new(None));
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("volume_phase_job"),
            PhaseJob {
                project: app.session.project_context,
                id,
                name,
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
    let (mut draft, stale) = phase_draft(app, id, volume);
    let name = app.doc.datasets[di].display_name();
    let fixed = volume.fixed_axis;
    let axes = std::array::from_fn::<_, 3, _>(|axis| {
        let nucleus = volume.source.axes()[axis]
            .nucleus
            .as_deref()
            .unwrap_or("unknown");
        (
            format!("F{} · {nucleus} · {}", axis + 1, axis_role(axis, fixed)),
            volume.processed.supports_phase(axis),
            volume.processed.phase_pivot_scale(axis),
        )
    });
    // Once reset to the current baseline, hidden edits must not return on Undo.
    if stale {
        discard_phase_draft(app, id);
    }
    let job = host
        .ctx()
        .data(|data| data.get_temp::<PhaseJob>(egui::Id::new("volume_phase_job")));
    let busy = app.session.ui.nmr_volume_phase_busy;
    let owns_target = phase_target(app) == Some(id);
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
            task_card::header(ui, area_id, "Phase", Some(name.as_str()), |ui| {
                let response = crate::ui::panel_chrome::close(ui, "Close Phase")
                    .on_hover_text("Hide Phase. Keeps the draft and lets any calculation continue.");
                if response.clicked() { close = true; }
            });
            if collapsed { return; }
            egui::ScrollArea::vertical().max_height(geometry.body_height).show(ui, |ui| {
                ui.label("Applies to the entire 3D spectrum. The current plane is for observation.");
                ui.small("PH0 and PH1 set the correction on the source data. Apply replaces the previous settings. An absorptive spectrum has not been independently confirmed.");
                if stale {
                    ui.label("The source or committed phase changed. Editing starts from the current settings.");
                }
                if draft.phases != committed {
                    ui.strong("Changes have not been applied. No live preview.");
                }
                if let Some(job) = &job {
                    ui.spinner();
                    let action = if job.cancellation.is_cancelled() { "Stopping" } else { "Calculating phase for" };
                    ui.label(format!("{action} {}{}", job.name, if job.id == id { " (this spectrum)" } else { " (another spectrum)" }));
                    ui.small("Closing Phase leaves the calculation running.");
                    let stop = commands::describe(app, CommandId::CancelNmrVolumePhase);
                    if ui.add_enabled(stop.enabled, egui::Button::new("Stop calculation")).clicked() {
                        commands::execute_without_clipboard(CommandId::CancelNmrVolumePhase, app, ui.ctx());
                    }
                }
                if !owns_target {
                    ui.small("Select this spectrum to edit its phase, or open Phase for the current spectrum.");
                }
                ui.push_id(id, |ui| ui.add_enabled_ui(!busy && owns_target, |ui| {
                    for (axis, (label, supported, scale)) in axes.iter().enumerate() {
                        ui.separator();
                        ui.strong(label);
                        if !supported {
                            ui.small("Phase unavailable: this axis needs a complex frequency-domain signal.");
                            continue;
                        }
                        let phase = &mut draft.phases[axis];
                        for (label, value) in [("PH0", &mut phase.p0_degrees), ("PH1", &mut phase.p1_degrees)] {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                let mut next = *value;
                                if ui.add(DragValue::new(&mut next).speed(0.1).suffix("°")).changed() && next.is_finite() {
                                    *value = next;
                                }
                            });
                        }
                        match scale.as_ref().map_err(Clone::clone).and_then(|scale| scale.ppm(phase.pivot_fraction).map(|ppm| (scale, ppm))) {
                            Ok((scale, mut ppm)) => {
                                ui.horizontal(|ui| {
                                    ui.label("Pivot");
                                    if ui.add(DragValue::new(&mut ppm).speed(0.001).max_decimals(8).suffix(" ppm")).changed()
                                        && let Err(error) = scale.fraction(ppm).and_then(|q| phase.repivot(q))
                                    {
                                        app.session.status = format!("Cannot move pivot for {name}: {error}");
                                    }
                                });
                            }
                            Err(error) => { ui.small(format!("Pivot unavailable: {error}")); }
                        }
                    }
                }));
                let dirty = draft.phases != committed;
                if ui.add_enabled(!busy && dirty, egui::Button::new("Discard changes")).clicked()
                    || (!busy && !dirty && !stale)
                {
                    discard_phase_draft(app, id);
                } else if dirty {
                    app.session.ui.nmr_volume_phase_drafts.insert(id, draft);
                }
                let desc = commands::describe(app, CommandId::ApplyNmrVolumePhase);
                if ui.add_enabled(desc.enabled && owns_target, egui::Button::new("Apply"))
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
    project: plotium_core::state::ProjectContextId,
    id: plotium_core::state::DatasetId,
    name: String,
    expected: [plotium_processing::nmr_volume::VolumePhase; 3],
    cancellation: nmr::CancellationToken,
    output:
        std::sync::Arc<std::sync::Mutex<Option<Result<plotium_core::state::Nmr2DDataset, String>>>>,
}
pub(super) fn poll_phase(app: &mut PlotiumApp, ctx: &egui::Context) {
    let key = egui::Id::new("volume_phase_job");
    let Some(job) = ctx.data(|data| data.get_temp::<PhaseJob>(key)) else {
        return;
    };
    if job.project != app.session.project_context {
        job.cancellation.cancel();
        ctx.data_mut(|data| data.remove::<PhaseJob>(key));
        app.session.ui.nmr_volume_phase_busy = false;
        return;
    }
    let result = match job.output.lock() {
        Ok(mut output) => output.take(),
        Err(error) => Some(Err(format!("NMR phase worker failed: {error}"))),
    };
    let Some(result) = result else { return };
    ctx.data_mut(|data| data.remove::<PhaseJob>(key));
    app.session.ui.nmr_volume_phase_busy = false;
    if job.cancellation.is_cancelled() {
        app.session.status = format!("Stopped phase calculation for {}. Draft kept.", job.name);
        return;
    }
    let result =
        result.and_then(|dataset| app.install_nmr_volume_result(job.id, job.expected, dataset));
    app.session.status = match result {
        Ok(()) => {
            app.session.ui.nmr_volume_phase_drafts.remove(&job.id);
            format!("Applied phase to {}", job.name)
        }
        Err(error) => format!("Cannot apply phase to {}: {error}", job.name),
    };
}

pub(super) fn cancel_phase(app: &mut PlotiumApp, ctx: &egui::Context) {
    let key = egui::Id::new("volume_phase_job");
    if ctx
        .data(|data| data.get_temp::<PhaseJob>(key))
        .is_some_and(|job| job.project != app.session.project_context)
    {
        poll_phase(app, ctx);
        return;
    }
    if let Some(job) = ctx.data(|data| data.get_temp::<PhaseJob>(egui::Id::new("volume_phase_job")))
    {
        job.cancellation.cancel();
        app.session.status = format!("Stopping phase calculation for {}…", job.name);
    }
}

pub(super) fn phase_gate(app: &PlotiumApp) -> Result<(), &'static str> {
    if app.session.ui.nmr_volume_phase_busy {
        return Err(
            "Wait for phase processing to finish or cancel it before applying new settings.",
        );
    }
    volume_phase_gate(app)?;
    let id = phase_target(app).expect("validated phase target");
    let data = &app.doc.datasets[app.doc.dataset_index(id).expect("validated phase dataset")];
    let volume = data
        .as_nmr2d()
        .and_then(|n| n.volume.as_ref())
        .expect("validated volume");
    let draft = app
        .session
        .ui
        .nmr_volume_phase_drafts
        .get(&data.resource_id())
        .ok_or("Open Phase and edit a phase setting before applying.")?;
    if !draft_matches(
        draft,
        &std::sync::Arc::downgrade(volume.source.dataset()),
        volume.phases,
    ) {
        return Err(
            "The source or committed phase changed. Edit the current settings in Phase before applying.",
        );
    }
    if draft.phases == volume.phases {
        return Err("Edit a phase setting before applying.");
    }
    if draft.phases.iter().any(|phase| phase.validate().is_err()) {
        return Err("Enter finite phase angles and a valid pivot before applying.");
    }
    if draft.phases.iter().enumerate().any(|(axis, phase)| {
        !volume.processed.supports_phase(axis)
            && (phase.p0_degrees != 0.0 || phase.p1_degrees != 0.0)
    }) {
        return Err("Apply nonzero phase only to complex frequency-domain signal axes.");
    }
    Ok(())
}

fn phase_draft(
    app: &PlotiumApp,
    id: plotium_core::state::DatasetId,
    volume: &plotium_core::state::NmrVolumeState,
) -> (plotium_core::state::NmrVolumePhaseDraft, bool) {
    let source = std::sync::Arc::downgrade(volume.source.dataset());
    let saved = app.session.ui.nmr_volume_phase_drafts.get(&id);
    if let Some(draft) = saved
        && draft_matches(draft, &source, volume.phases)
    {
        return (draft.clone(), false);
    }
    (
        plotium_core::state::NmrVolumePhaseDraft {
            source,
            baseline: volume.phases,
            phases: volume.phases,
        },
        saved.is_some(),
    )
}

fn discard_phase_draft(app: &mut PlotiumApp, id: plotium_core::state::DatasetId) {
    app.session.ui.nmr_volume_phase_drafts.remove(&id);
}

fn draft_matches(
    draft: &plotium_core::state::NmrVolumePhaseDraft,
    source: &std::sync::Weak<nmr::Dataset>,
    phases: [plotium_processing::nmr_volume::VolumePhase; 3],
) -> bool {
    draft.source.ptr_eq(source) && draft.baseline == phases
}

pub(super) fn open_phase_gate(app: &PlotiumApp) -> Result<(), &'static str> {
    let id = phase_target(app).ok_or("Select an NMR spectrum before opening Phase.")?;
    let dataset = app
        .doc
        .dataset_index(id)
        .and_then(|di| app.doc.datasets.get(di))
        .ok_or("Select an NMR spectrum before opening Phase.")?;
    if !matches!(dataset, Dataset::Nmr(_) | Dataset::Nmr2D(_)) {
        return Err("Select an NMR spectrum before opening Phase.");
    }
    if let Some(volume) = dataset.as_nmr2d().and_then(|n| n.volume.as_ref()) {
        return require_volume_phase(volume);
    }
    let axis = dataset.active_phase_axis(app.session.ui.phase_axis);
    if !canvas_axis_phaseable(dataset, axis) {
        return Err(
            "Enable a Phase step on the current frequency-domain axis, or choose another axis in Processing.",
        );
    }
    if canvas_phase_target(app) != Some(id) {
        return Err(
            "Select a plot of the chosen spectrum on the active canvas before opening Phase.",
        );
    }
    Ok(())
}

pub(super) fn canvas_axis_phaseable(
    dataset: &Dataset,
    axis: plotium_core::state::PhaseAxis,
) -> bool {
    dataset.pivot_ppm(axis).is_some()
        && dataset.axis_pipeline(axis).is_some_and(|pipeline| {
            pipeline.steps.iter().any(|step| {
                step.enabled && matches!(step.kind, plotium_processing::StepKind::Phase(_))
            })
        })
}

fn volume_phase_gate(app: &PlotiumApp) -> Result<(), &'static str> {
    let volume = phase_target(app)
        .and_then(|id| app.doc.dataset_index(id))
        .and_then(|di| app.doc.datasets.get(di))
        .and_then(Dataset::as_nmr2d)
        .and_then(|n| n.volume.as_ref())
        .ok_or("Select a three-dimensional NMR spectrum before applying phase.")?;
    require_volume_phase(volume)
}

fn require_volume_phase(volume: &plotium_core::state::NmrVolumeState) -> Result<(), &'static str> {
    if !(0..3).any(|axis| volume.processed.supports_phase(axis)) {
        return Err(
            "Select a spectrum with real and imaginary components; these axes cannot be phased.",
        );
    }
    Ok(())
}

fn axis_role(axis: usize, fixed: usize) -> &'static str {
    // The plane descriptor keeps the surviving acquisition axes in order:
    // rows are vertical and the faster-varying columns are horizontal.
    if axis == fixed {
        "fixed axis"
    } else if axis == usize::from(fixed == 0) {
        "vertical axis"
    } else {
        "horizontal axis"
    }
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
                | CommandId::Integrate
                | CommandId::Tool(Tool::Integrate | Tool::Symmetry)
        )
    {
        return Err(
            "Open a standalone spectrum for peak or integral editing; use Phase to adjust volume phase.",
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "nmr_volume_tests.rs"]
mod tests;
