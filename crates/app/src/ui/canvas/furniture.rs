use super::*;
use plotium_core::properties::{PropertyAddress, guide};

enum FurnitureHit {
    Legend {
        object: ObjectId,
        rect: PlotRect,
    },
    ColorScale {
        object: ObjectId,
    },
    RegionLabel {
        object: ObjectId,
        dataset: plotium_core::state::DatasetId,
        region: RegionId,
        rect: PlotRect,
    },
    PlotLabel {
        object: ObjectId,
        label: super::plot_labels::LabelHit,
    },
}

impl FurnitureHit {
    fn object(&self) -> ObjectId {
        match self {
            Self::Legend { object, .. }
            | Self::ColorScale { object, .. }
            | Self::RegionLabel { object, .. }
            | Self::PlotLabel { object, .. } => *object,
        }
    }
}

pub(crate) fn handle_furniture_interactions(
    app: &mut PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    ui: &Ui,
) -> bool {
    if !matches!(app.session.tool, Tool::Select | Tool::Regions) {
        return false;
    }
    let (pointer, down, pressed, released, double_clicked, escape) = ui.input(|input| {
        (
            input.pointer.hover_pos(),
            input.pointer.primary_down(),
            input.pointer.primary_pressed(),
            input.pointer.primary_released(),
            input
                .pointer
                .button_double_clicked(egui::PointerButton::Primary),
            input.key_pressed(egui::Key::Escape),
        )
    });

    if matches!(app.interaction(), Interaction::Furniture(_)) {
        if escape {
            app.cancel_interaction();
            app.session.status = "Restored the previous label position.".to_owned();
            return true;
        }
        if down && let Some(pointer) = pointer {
            update_furniture_drag(app, ci, canvas_rect, pointer);
        }
        if released || !down {
            finish_furniture_drag(app);
            // egui reports a double-click on the release that ends this drag.
            if double_clicked
                && let Some(pointer) = pointer
                && let Some(hit) = furniture_hit(app, ci, canvas_rect, pointer)
            {
                reset_furniture_position(app, ci, hit);
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return true;
    }

    let Some(pointer) = pointer else {
        return false;
    };
    let Some(hit) = furniture_hit(app, ci, canvas_rect, pointer) else {
        return false;
    };
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    let object = hit.object();
    if double_clicked {
        app.select_object(ci, object);
        if matches!(
            &hit,
            FurnitureHit::Legend { .. } | FurnitureHit::ColorScale { .. }
        ) {
            app.session.ui.requested_inspector_section =
                Some(crate::ui::properties::panel::GUIDE_SECTION.to_owned());
        }
        reset_furniture_position(app, ci, hit);
        return true;
    }
    if !pressed {
        return true;
    }

    app.select_object(ci, object);
    if matches!(
        &hit,
        FurnitureHit::Legend { .. } | FurnitureHit::ColorScale { .. }
    ) {
        app.session.ui.requested_inspector_section =
            Some(crate::ui::properties::panel::GUIDE_SECTION.to_owned());
    }
    if matches!(&hit, FurnitureHit::ColorScale { .. }) {
        return true;
    }
    freeze_board_for_gesture(app);
    let target = match hit {
        FurnitureHit::Legend { rect, .. } => {
            let before = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
                .map(|plot| plot.axis_overrides.clone())
                .unwrap_or_default();
            FurnitureTarget::Legend {
                before: Box::new(before),
                grab_offset: [pointer.x - rect.left, pointer.y - rect.top],
            }
        }
        FurnitureHit::ColorScale { .. } => return true,
        FurnitureHit::PlotLabel { label, .. } => {
            let before = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
                .map(|plot| plot.axis_overrides.clone())
                .unwrap_or_default();
            FurnitureTarget::PlotLabel {
                before: Box::new(before),
                annotation: label.annotation,
                grab_offset: [pointer.x - label.origin[0], pointer.y - label.origin[1]],
            }
        }
        FurnitureHit::RegionLabel {
            dataset,
            region,
            rect,
            ..
        } => {
            let before = app
                .doc
                .dataset_index(dataset)
                .and_then(|index| app.doc.datasets[index].region_analysis())
                .map(|state| state.regions.clone())
                .unwrap_or_default();
            FurnitureTarget::RegionLabel {
                dataset,
                region,
                before,
                grab_offset: [
                    pointer.x - (rect.left + rect.width * 0.5),
                    pointer.y - (rect.top + rect.height * 0.5),
                ],
            }
        }
    };
    app.begin_interaction(Interaction::Furniture(FurnitureDrag {
        canvas: ci,
        object,
        target,
    }));
    true
}

fn furniture_hit(
    app: &PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    pointer: Pos2,
) -> Option<FurnitureHit> {
    let canvas = app.doc.canvases.get(ci)?;
    for object in canvas
        .objects
        .iter()
        .rev()
        .filter(|object| object.visible && !object.locked)
    {
        let Some(plot_object) = object.plot() else {
            continue;
        };
        let Some((plot, scale)) = plot_geometry(app, ci, object.id, canvas_rect) else {
            continue;
        };
        let figure = plot_object.figure();
        if let Some(rect) = plotium_render::legend_rect(figure, plot, scale)
            && rect_contains(rect, pointer, 2.0)
        {
            return Some(FurnitureHit::Legend {
                object: object.id,
                rect,
            });
        }
        if let Some(rect) = plotium_render::color_scale_rect(figure, plot, scale)
            && rect_contains(rect, pointer, 3.0)
        {
            return Some(FurnitureHit::ColorScale { object: object.id });
        }
        // Labels are clipped to the plot, so only its inside can grab one.
        if rect_contains(plot, pointer, 0.0)
            && let Some(label) = super::plot_labels::label_hit(plot_object, plot, scale, |rect| {
                rect_contains(rect, pointer, 3.0)
            })
        {
            return Some(FurnitureHit::PlotLabel {
                object: object.id,
                label,
            });
        }
        let Some(dataset) = object.dataset() else {
            continue;
        };
        for annotation in figure.range_annotations.iter().rev() {
            let x0 = to_screen_x(annotation.x0, plot, &figure.x);
            let x1 = to_screen_x(annotation.x1, plot, &figure.x);
            let Some(layout) = plotium_render::range_label_layout(
                plot,
                x0.min(x1),
                x0.max(x1),
                figure.typography.tick_pt * scale,
                &annotation.label,
                annotation.label_position,
            ) else {
                continue;
            };
            let rect = layout.rect(figure.typography.tick_pt * scale);
            if rect_contains(rect, pointer, 3.0) {
                return Some(FurnitureHit::RegionLabel {
                    object: object.id,
                    dataset,
                    region: RegionId::new(annotation.source_id),
                    rect,
                });
            }
        }
    }
    None
}

pub(crate) fn furniture_hovered(
    app: &PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    pointer: Pos2,
) -> bool {
    matches!(app.session.tool, Tool::Select | Tool::Regions)
        && furniture_hit(app, ci, canvas_rect, pointer).is_some()
}

fn plot_geometry(
    app: &PlotiumApp,
    ci: usize,
    object: ObjectId,
    canvas_rect: egui::Rect,
) -> Option<(PlotRect, f32)> {
    let outer = content_screen_rect(
        app.session.board,
        app.doc.canvases.get(ci)?,
        object,
        canvas_rect,
    )?;
    let figure = app.doc.canvases[ci].object(object)?.plot()?.figure();
    let scale = app.session.board.zoom;
    let plot = plot_screen_box(&app.doc.canvases[ci], object, figure, outer, scale);
    Some((plot, scale))
}

fn update_furniture_drag(app: &mut PlotiumApp, ci: usize, canvas_rect: egui::Rect, pointer: Pos2) {
    let Interaction::Furniture(drag) = app.interaction() else {
        return;
    };
    if drag.canvas != ci {
        return;
    }
    let object = drag.object;
    let target = drag.target.clone();
    let Some((plot, scale)) = plot_geometry(app, ci, object, canvas_rect) else {
        return;
    };
    match target {
        FurnitureTarget::Legend { grab_offset, .. } => {
            let Some(plot_object) = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
            else {
                return;
            };
            let Some(position) = plotium_render::legend_position_for_origin(
                plot_object.figure(),
                plot,
                scale,
                [pointer.x - grab_offset[0], pointer.y - grab_offset[1]],
            ) else {
                return;
            };
            let mut overrides = plot_object.axis_overrides.clone();
            overrides.set_legend_position(position);
            app.set_axis_overrides_value(ci, object, &overrides);
        }
        FurnitureTarget::PlotLabel {
            annotation,
            grab_offset,
            ..
        } => {
            let Some(overrides) = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
                .and_then(|plot_object| {
                    super::plot_labels::moved_label(
                        plot_object,
                        plot,
                        scale,
                        annotation,
                        [pointer.x - grab_offset[0], pointer.y - grab_offset[1]],
                    )
                })
            else {
                return;
            };
            app.set_axis_overrides_value(ci, object, &overrides);
        }
        FurnitureTarget::RegionLabel {
            dataset,
            region,
            grab_offset,
            ..
        } => {
            let Some(index) = app.doc.dataset_index(dataset) else {
                return;
            };
            let center = [pointer.x - grab_offset[0], pointer.y - grab_offset[1]];
            let position = [
                ((center[0] - plot.left) / plot.width).clamp(0.0, 1.0),
                ((center[1] - plot.top) / plot.height).clamp(0.0, 1.0),
            ];
            if let Some(item) = app.doc.datasets[index]
                .region_analysis_mut()
                .and_then(|state| state.regions.iter_mut().find(|item| item.id == region))
            {
                item.label_position = Some(position);
                app.rebuild_canvases_for(index);
            }
        }
    }
}

fn finish_furniture_drag(app: &mut PlotiumApp) {
    let Interaction::Furniture(drag) = app.take_interaction() else {
        return;
    };
    match drag.target {
        FurnitureTarget::Legend { before, .. } => {
            let Some(after) = app.doc.canvases[drag.canvas]
                .object(drag.object)
                .and_then(|object| object.plot())
                .map(|plot| plot.axis_overrides.clone())
            else {
                return;
            };
            // A click without a drag moves nothing and decides nothing.
            if after == *before {
                return;
            }
            let action = Action::set_axis_overrides(drag.canvas, drag.object, *before, after);
            let placed = legend_placement(app, drag.canvas, drag.object);
            app.execute_hand_edit(action, &placed, true);
            app.session.status =
                "Moved legend. Double-click it to restore automatic placement.".into();
        }
        FurnitureTarget::PlotLabel { before, .. } => {
            let Some(after) = app.doc.canvases[drag.canvas]
                .object(drag.object)
                .and_then(|object| object.plot())
                .map(|plot| plot.axis_overrides.clone())
            else {
                return;
            };
            if after == *before {
                return;
            }
            app.execute_action(Action::set_axis_overrides(
                drag.canvas,
                drag.object,
                *before,
                after,
            ));
            app.session.status =
                "Moved label. Double-click it to restore automatic placement.".into();
        }
        FurnitureTarget::RegionLabel {
            dataset, before, ..
        } => {
            let Some(index) = app.doc.dataset_index(dataset) else {
                return;
            };
            let after = app.doc.datasets[index]
                .region_analysis()
                .map(|state| state.regions.clone())
                .unwrap_or_default();
            app.execute_action(Action::set_regions(dataset, before, after));
            app.session.status =
                "Moved region label. Double-click it to restore automatic placement.".into();
        }
    }
}

fn reset_furniture_position(app: &mut PlotiumApp, ci: usize, hit: FurnitureHit) {
    match hit {
        FurnitureHit::Legend { object, .. } => {
            let Some(before) = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
                .map(|plot| plot.axis_overrides.clone())
            else {
                return;
            };
            let mut after = before.clone();
            after.reset_guide_placement();
            let action = Action::set_axis_overrides(ci, object, before, after);
            let placed = legend_placement(app, ci, object);
            app.execute_hand_edit(action, &placed, false);
            app.session.status = "Restored automatic legend placement.".into();
        }
        FurnitureHit::ColorScale { object, .. } => {
            let Some(before) = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
                .map(|plot| plot.axis_overrides.clone())
            else {
                return;
            };
            let mut after = before.clone();
            after.guide_placement = None;
            app.execute_action(Action::set_axis_overrides(ci, object, before, after));
            app.session.status = "Restored automatic colour-scale placement.".into();
        }
        FurnitureHit::PlotLabel { object, label } => {
            let Some(plot_object) = app.doc.canvases[ci]
                .object(object)
                .and_then(|object| object.plot())
            else {
                return;
            };
            let before = plot_object.axis_overrides.clone();
            let after = super::plot_labels::auto_label(plot_object, label.annotation);
            if after != before {
                app.execute_action(Action::set_axis_overrides(ci, object, before, after));
            }
            app.session.status = "Restored automatic label placement.".into();
        }
        FurnitureHit::RegionLabel {
            dataset, region, ..
        } => {
            let Some(index) = app.doc.dataset_index(dataset) else {
                return;
            };
            let before = app.doc.datasets[index]
                .region_analysis()
                .map(|state| state.regions.clone())
                .unwrap_or_default();
            let mut after = before.clone();
            if let Some(item) = after.iter_mut().find(|item| item.id == region) {
                item.label_position = None;
            }
            app.execute_action(Action::set_regions(dataset, before, after));
            app.session.status = "Restored automatic region-label placement.".into();
        }
    }
}

/// The legend position a drag sets by hand; resetting it also forgets a
/// hand-picked placement, which stores the same decision.
fn legend_placement(app: &PlotiumApp, ci: usize, object: ObjectId) -> Vec<PropertyAddress> {
    app.object_target(ci, object)
        .map(|target| vec![PropertyAddress::new(target, guide::POSITION)])
        .unwrap_or_default()
}

fn rect_contains(rect: PlotRect, pointer: Pos2, padding: f32) -> bool {
    pointer.x >= rect.left - padding
        && pointer.x <= rect.right() + padding
        && pointer.y >= rect.top - padding
        && pointer.y <= rect.bottom() + padding
}
