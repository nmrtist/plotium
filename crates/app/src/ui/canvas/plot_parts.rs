//! The selected plot part (legend or label) and arrow-key nudging of it, or of
//! the selected objects and panels when no part is selected.
use super::*;
use plotium_core::properties::{PropertyAddress, guide};
use plotium_core::state::{PendingNudge, PlotPart, SelectionScope};

/// One arrow press moves this far in pt; Shift moves `NUDGE_FAST` times as far.
const NUDGE_PT: f32 = 0.5;
const NUDGE_FAST: f32 = 10.0;

const ARROWS: [egui::Key; 4] = [
    egui::Key::ArrowLeft,
    egui::Key::ArrowRight,
    egui::Key::ArrowUp,
    egui::Key::ArrowDown,
];

/// The screen box of `part` of plot `object`.
fn part_rect(
    app: &PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    object: ObjectId,
    part: PlotPart,
) -> Option<PlotRect> {
    let (plot, scale) = super::furniture::plot_geometry(app, ci, object, canvas_rect)?;
    let plot_object = app.doc.canvases.get(ci)?.object(object)?.plot()?;
    let figure = plot_object.figure();
    match part {
        PlotPart::Legend => {
            let stack = app.doc.canvases[ci].shared_x_stack_placement(object);
            plotium_render::legend_rect_placed(figure, plot, scale, stack.as_ref())
        }
        PlotPart::Label(id) => super::plot_labels::label_bounds(plot_object, plot, scale, id),
        PlotPart::Annotation(id) => super::plot_marks::mark_bounds(plot_object, plot, id),
        PlotPart::RegionLabel { region, .. } => {
            super::furniture::region_label_rect(figure, region, plot, scale)
        }
    }
}

/// Outline the selected part, so the user sees what the arrow keys move.
pub(crate) fn paint_plot_part_selection(
    app: &PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    painter: &egui::Painter,
    chrome: ChromeStyle,
) {
    let Some((selected_ci, selected)) = app.selected_plot_part() else {
        return;
    };
    if selected_ci != ci {
        return;
    }
    let Some(rect) = part_rect(app, ci, canvas_rect, selected.object, selected.part) else {
        return;
    };
    painter.rect_stroke(
        plot_rect(rect).expand(2.0),
        0.0,
        Stroke::new(1.0_f32, chrome.selection_active),
        StrokeKind::Outside,
    );
    // Handles on a selected arrow's ends show that each end drags on its own.
    if let PlotPart::Annotation(id) = selected.part
        && let Some((plot, _)) =
            super::furniture::plot_geometry(app, ci, selected.object, canvas_rect)
        && let Some(ends) = app.doc.canvases[ci]
            .object(selected.object)
            .and_then(|object| object.plot())
            .and_then(|plot_object| super::plot_marks::arrow_ends(plot_object, plot, id))
    {
        for end in ends {
            painter.circle_filled(end, 3.0, chrome.selection_active);
        }
    }
}

/// Arrow keys move the selected plot part, else the selected objects and
/// panels, by `NUDGE_PT` (Shift: ten times as far). A held key is one undo step.
pub(crate) fn handle_nudge_keys(app: &mut PlotiumApp, ci: usize, canvas_rect: egui::Rect, ui: &Ui) {
    let (step, held) = ui.input(|input| {
        let mut step = [0.0f32, 0.0f32];
        for (key, axis, sign) in [
            (egui::Key::ArrowLeft, 0, -1.0),
            (egui::Key::ArrowRight, 0, 1.0),
            (egui::Key::ArrowUp, 1, -1.0),
            (egui::Key::ArrowDown, 1, 1.0),
        ] {
            if input.key_pressed(key) && (input.modifiers.is_none() || input.modifiers.shift_only())
            {
                step[axis] += sign;
            }
        }
        let size = if input.modifiers.shift {
            NUDGE_PT * NUDGE_FAST
        } else {
            NUDGE_PT
        };
        (
            [step[0] * size, step[1] * size],
            ARROWS.iter().any(|key| input.key_down(*key)),
        )
    });
    if !held || app.interaction().is_active() {
        commit_nudge(app);
    }
    if step == [0.0, 0.0]
        || !app.session.tool.is_layout_tool()
        || app.session.ui.selection_scope != SelectionScope::CanvasObjects
        || app.interaction().is_active()
        // A selected processing region owns the horizontal arrows.
        || app.session.ui.craft_selected_region.is_some()
        || ui.ctx().egui_wants_keyboard_input()
    {
        return;
    }
    let moved = if let Some((part_ci, selected)) = app.selected_plot_part() {
        part_ci == ci && nudge_part(app, ci, canvas_rect, selected.object, selected.part, step)
    } else {
        nudge_frames(app, ci, step)
    };
    if moved {
        let what = match app.session.ui.pending_nudge {
            Some(PendingNudge::Plot {
                part: PlotPart::Legend,
                ..
            }) => "legend",
            Some(PendingNudge::Plot {
                part: PlotPart::Annotation(_),
                ..
            }) => "annotation",
            Some(PendingNudge::Plot { .. } | PendingNudge::Regions { .. }) => "label",
            _ => "selection",
        };
        let distance = step[0].abs().max(step[1].abs());
        app.session.status = format!(
            "Moved {what} {distance} pt. Shift+arrow moves {} pt.",
            NUDGE_PT * NUDGE_FAST
        );
    }
}

/// Record a finished nudge as one undo step.
fn commit_nudge(app: &mut PlotiumApp) {
    let Some(pending) = app.session.ui.pending_nudge.take() else {
        return;
    };
    match pending {
        PendingNudge::Plot {
            canvas,
            object,
            part,
            before,
        } => {
            let Some(after) = app
                .doc
                .canvases
                .get(canvas)
                .and_then(|page| page.object(object))
                .and_then(|item| item.plot())
                .map(|plot| plot.axis_overrides.clone())
            else {
                return;
            };
            if after == *before {
                return;
            }
            let action = Action::set_axis_overrides(canvas, object, *before, after);
            if part == PlotPart::Legend {
                // The same hand edit a legend drag records.
                let placed: Vec<_> = app
                    .object_target(canvas, object)
                    .map(|target| vec![PropertyAddress::new(target, guide::POSITION)])
                    .unwrap_or_default();
                app.execute_hand_edit(action, &placed, true);
            } else {
                app.execute_action(action);
            }
        }
        PendingNudge::Regions {
            dataset, before, ..
        } => {
            let Some(after) = app
                .doc
                .dataset_by_id(dataset)
                .and_then(Dataset::region_analysis)
                .map(|state| state.regions.clone())
            else {
                return;
            };
            if after != before {
                app.execute_action(Action::set_regions(dataset, before, after));
            }
        }
        PendingNudge::Frames { canvas, before } => {
            let Some(page) = app.doc.canvases.get(canvas) else {
                return;
            };
            let (before, after): (Vec<_>, Vec<_>) = before
                .into_iter()
                .filter_map(|(id, old)| {
                    page.layout_frame(id)
                        .filter(|new| *new != old)
                        .map(|new| ((id, old), (id, new)))
                })
                .unzip();
            if !before.is_empty() {
                app.execute_action(Action::set_object_frames(canvas, before, after));
            }
        }
    }
}

/// Whether `pending` is a nudge of the target about to move; a nudge of
/// anything else is recorded first so each target keeps its own undo step.
fn continues(pending: &PendingNudge, next: &PendingNudge) -> bool {
    match (pending, next) {
        (
            PendingNudge::Plot {
                canvas,
                object,
                part,
                ..
            },
            PendingNudge::Plot {
                canvas: c,
                object: o,
                part: p,
                ..
            },
        ) => canvas == c && object == o && part == p,
        (
            PendingNudge::Regions {
                dataset, region, ..
            },
            PendingNudge::Regions {
                dataset: d,
                region: r,
                ..
            },
        ) => dataset == d && region == r,
        (
            PendingNudge::Frames { canvas, before },
            PendingNudge::Frames {
                canvas: c,
                before: b,
            },
        ) => {
            canvas == c && before.len() == b.len() && before.iter().zip(b).all(|(x, y)| x.0 == y.0)
        }
        _ => false,
    }
}

/// Start (or continue) a pending nudge from the state `next` captures.
fn begin_nudge(app: &mut PlotiumApp, next: PendingNudge) {
    if app
        .session
        .ui
        .pending_nudge
        .as_ref()
        .is_some_and(|pending| continues(pending, &next))
    {
        return;
    }
    commit_nudge(app);
    app.session.ui.pending_nudge = Some(next);
}

fn nudge_part(
    app: &mut PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    object: ObjectId,
    part: PlotPart,
    step_pt: [f32; 2],
) -> bool {
    let Some((plot, scale)) = super::furniture::plot_geometry(app, ci, object, canvas_rect) else {
        return false;
    };
    let delta = [step_pt[0] * scale, step_pt[1] * scale];
    let Some(plot_object) = app.doc.canvases[ci].object(object).and_then(|o| o.plot()) else {
        return false;
    };
    match part {
        PlotPart::Legend => {
            let figure = plot_object.figure();
            let stack = app.doc.canvases[ci].shared_x_stack_placement(object);
            let Some(position) =
                plotium_render::legend_rect_placed(figure, plot, scale, stack.as_ref()).and_then(
                    |r| {
                        plotium_render::legend_position_for_origin(
                            figure,
                            plot,
                            scale,
                            [r.left + delta[0], r.top + delta[1]],
                        )
                    },
                )
            else {
                return false;
            };
            let before = plot_object.axis_overrides.clone();
            let mut after = before.clone();
            after.set_legend_position(position);
            begin_nudge(
                app,
                PendingNudge::Plot {
                    canvas: ci,
                    object,
                    part,
                    before: Box::new(before),
                },
            );
            app.set_axis_overrides_value(ci, object, &after);
            true
        }
        PlotPart::Label(annotation) => {
            let before = plot_object.axis_overrides.clone();
            let Some(after) =
                super::plot_labels::nudged_label(plot_object, plot, scale, annotation, delta)
            else {
                return false;
            };
            begin_nudge(
                app,
                PendingNudge::Plot {
                    canvas: ci,
                    object,
                    part,
                    before: Box::new(before),
                },
            );
            app.set_axis_overrides_value(ci, object, &after);
            true
        }
        PlotPart::Annotation(annotation) => {
            let before = plot_object.axis_overrides.clone();
            let Some(after) = super::plot_marks::moved_mark(
                plot_object,
                plot,
                annotation,
                plotium_core::state::MarkGrip::Whole,
                &before.annotations,
                delta,
            ) else {
                return false;
            };
            begin_nudge(
                app,
                PendingNudge::Plot {
                    canvas: ci,
                    object,
                    part,
                    before: Box::new(before),
                },
            );
            app.set_axis_overrides_value(ci, object, &after);
            true
        }
        PlotPart::RegionLabel { dataset, region } => {
            let Some(rect) =
                super::furniture::region_label_rect(plot_object.figure(), region, plot, scale)
            else {
                return false;
            };
            let Some(index) = app.doc.dataset_index(dataset) else {
                return false;
            };
            let Some(before) = app.doc.datasets[index]
                .region_analysis()
                .map(|state| state.regions.clone())
            else {
                return false;
            };
            // The stored position is the label centre as a fraction of the plot.
            let position = [
                ((rect.left + rect.width * 0.5 + delta[0] - plot.left) / plot.width)
                    .clamp(0.0, 1.0),
                ((rect.top + rect.height * 0.5 + delta[1] - plot.top) / plot.height)
                    .clamp(0.0, 1.0),
            ];
            begin_nudge(
                app,
                PendingNudge::Regions {
                    dataset,
                    region,
                    before,
                },
            );
            if let Some(item) = app.doc.datasets[index]
                .region_analysis_mut()
                .and_then(|state| state.regions.iter_mut().find(|item| item.id == region))
            {
                item.label_position = Some(position);
            }
            app.rebuild_canvases_for(index);
            true
        }
    }
}

/// The layout frames the selection moves: each selected loose object, and
/// each selected panel or panel holding a selected item, once. A panel moves
/// through any of its items, the way the object inspector's X and Y do.
fn nudge_targets(app: &PlotiumApp, ci: usize) -> Vec<ObjectId> {
    let Some(page) = app.doc.canvases.get(ci) else {
        return Vec::new();
    };
    let mut panels = Vec::new();
    let mut targets = Vec::new();
    let mut add = |id: ObjectId, panel: Option<PanelId>| {
        if let Some(panel) = panel {
            if panels.contains(&panel) {
                return;
            }
            panels.push(panel);
        }
        targets.push(id);
    };
    for &id in app.session.ui.selection.objects() {
        if page.object(id).is_some_and(|item| !item.locked) {
            add(id, page.parent_panel(id));
        }
    }
    for path in app.session.ui.hierarchical_selection.paths() {
        if path.canvas != page.resource_id || path.content.is_some() {
            continue;
        }
        if let Some(panel) = path.panel.and_then(|id| page.panel(id))
            && !panel.locked
            && let Some(&first) = panel.item_order.first()
        {
            add(first, Some(panel.id));
        }
    }
    targets
}

fn nudge_frames(app: &mut PlotiumApp, ci: usize, step: [f32; 2]) -> bool {
    let targets = nudge_targets(app, ci);
    let before: Vec<_> = targets
        .iter()
        .filter_map(|&id| {
            app.doc.canvases[ci]
                .layout_frame(id)
                .map(|frame| (id, frame))
        })
        .collect();
    if before.is_empty() {
        return false;
    }
    begin_nudge(
        app,
        PendingNudge::Frames {
            canvas: ci,
            before: before.clone(),
        },
    );
    for (id, frame) in before {
        let moved = ObjectFrame::new(
            frame.x + step[0],
            frame.y + step[1],
            frame.width,
            frame.height,
        );
        app.set_object_frame(ci, id, moved);
    }
    true
}

#[cfg(test)]
#[path = "plot_parts_tests.rs"]
mod tests;
