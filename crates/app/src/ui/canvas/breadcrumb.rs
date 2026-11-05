use super::*;

pub(crate) fn canvas_breadcrumb(app: &mut PlotiumApp, ci: usize, ui: &mut Ui) {
    let canvas_name = app.doc.canvases[ci].name.clone();
    let lead = app.session.ui.hierarchical_selection.lead();
    let panel = lead.and_then(|path| path.panel).and_then(|id| {
        app.doc.canvases[ci]
            .panel(id)
            .map(|panel| (id, panel.name.clone()))
    });
    let content = lead.and_then(|path| path.content).and_then(|id| {
        app.doc.canvases[ci]
            .object(id)
            .map(|object| (id, object.name.clone()))
    });
    let phase = (app.session.tool == Tool::ManualPhase).then(|| phase_target(app, ci));
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        if ui.small_button(canvas_name).clicked() {
            clicked = Some(BreadcrumbTarget::Canvas);
        }
        if let Some((id, name)) = panel {
            ui.weak("›");
            if ui.small_button(name).clicked() {
                clicked = Some(BreadcrumbTarget::Panel(id));
            }
        }
        if let Some((id, name)) = content {
            ui.weak("›");
            if ui.small_button(name).clicked() {
                clicked = Some(BreadcrumbTarget::Content(id));
            }
        } else if lead.is_none() && phase.is_none() {
            ui.weak("›");
            ui.small(app.session.tool.label());
        }
        if let Some(phase) = phase {
            ui.weak("›");
            ui.small(phase);
        }
    });
    if app.session.active_canvas == Some(ci)
        && let Some(feedback) = crate::ui::nmr_phase_preview::canvas_feedback(app, ui.ctx())
    {
        ui.small(feedback);
    }
    ui.add_space(2.0);
    match clicked {
        Some(BreadcrumbTarget::Canvas) => app.exit_panel_scope(),
        Some(BreadcrumbTarget::Panel(panel)) => app.select_panel(ci, panel),
        Some(BreadcrumbTarget::Content(content)) => app.select_content(ci, content),
        None => {}
    }
}

fn phase_target(app: &PlotiumApp, ci: usize) -> String {
    use plotium_core::state::PhaseAxis;
    let target = data_edit_target(app, ci)
        .and_then(|id| app.doc.canvases[ci].object(id))
        .and_then(|object| object.dataset())
        .and_then(|id| app.doc.dataset_index(id))
        .and_then(|di| app.doc.datasets.get(di));
    let Some(dataset) = target else {
        return "Phase · Select a spectrum plot".into();
    };
    if crate::ui::nmr_volume::phase_target(app) != Some(dataset.resource_id()) {
        return "Phase · Select a plot of the chosen spectrum".into();
    }
    let axis = dataset.active_phase_axis(app.session.ui.phase_axis);
    if let Some(volume) = dataset.as_nmr2d().and_then(|n| n.volume.as_ref()) {
        let id = dataset.resource_id();
        if let Some(draft) = app.session.ui.nmr_volume_phase_drafts.get(&id) {
            let a = draft.canvas_axis;
            let nucleus = volume.source.axes()[a]
                .nucleus
                .as_deref()
                .unwrap_or("unknown");
            return format!(
                "Phase · {} · F{} · {nucleus} · {} · Apply: entire volume",
                dataset.display_name(),
                a + 1,
                if a == volume.fixed_axis {
                    "fixed (numeric controls)"
                } else if axis == PhaseAxis::F1 {
                    "vertical"
                } else {
                    "horizontal"
                }
            );
        }
        return format!(
            "Phase · {} · choose an acquisition axis",
            dataset.display_name()
        );
    }
    let (nucleus, scope) = match dataset {
        Dataset::Nmr(nmr) => (nmr.data.axes().first(), "whole spectrum"),
        Dataset::Nmr2D(nmr) if nmr.volume.is_none() => (
            match axis {
                PhaseAxis::F1 => nmr.native_base.axes().first(),
                _ => nmr.native_base.axes().last(),
            },
            "all traces along this axis",
        ),
        _ => return "Phase · Select a phaseable spectrum plot".into(),
    };
    if !crate::ui::nmr_volume::canvas_axis_phaseable(dataset, axis) {
        return format!(
            "Phase · {} · No enabled frequency-domain phase step on {}",
            dataset.display_name(),
            axis.label()
        );
    }
    let nucleus = nucleus
        .and_then(|axis| axis.nucleus.as_deref())
        .unwrap_or("unknown");
    let direction = if axis == PhaseAxis::F1 {
        "vertical axis"
    } else {
        "horizontal axis"
    };
    format!(
        "Phase · {} · {} · {nucleus} · {direction} · {scope}",
        dataset.display_name(),
        axis.label()
    )
}

enum BreadcrumbTarget {
    Canvas,
    Panel(PanelId),
    Content(ObjectId),
}
