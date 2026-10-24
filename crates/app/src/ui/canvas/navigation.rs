use super::*;

#[path = "navigation_input.rs"]
mod input;
use input::*;
#[path = "navigation_gain.rs"]
mod gain;
use gain::scale_line_intensity;
pub(crate) use gain::{peak_height_label, supports_peak_height};
#[path = "navigation_pointer.rs"]
mod pointer;
pub(crate) use input::{last_wheel_was_trackpad, selected_plot_body, sync_macos_trackpad_gesture};
pub(crate) use pointer::*;

/// Wheel navigation targets the hovered plot; trackpad swipes and pinches target
/// the board unless over the selected plot's body. Cmd/Ctrl targets the board.
/// Returns whether consumed.
pub(crate) fn handle_navigation(
    app: &mut PlotiumApp,
    ci: usize,
    rect: egui::Rect,
    ui: &Ui,
) -> bool {
    let events = ui.input(|i| i.events.clone());
    let trackpad_gesture = macos_trackpad_gesture(ui.ctx(), &events, true);
    note_wheel_device(ui.ctx(), &events, trackpad_gesture.active);
    let (hover, navigation_input, delta, dbl) = ui.input(|i| {
        let native_pinch_delta = i
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Zoom(delta) => Some(*delta),
                _ => None,
            })
            .reduce(|combined, delta| combined * delta);
        // An active Point gesture started with a touch phase, unlike Point-unit
        // high-resolution mouse wheels. Read its raw two-axis displacement so
        // modifier changes cannot make egui reinterpret later frames as zoom.
        let trackpad_pan = trackpad_gesture.active && native_pinch_delta.is_none();
        let scroll_delta = if trackpad_pan {
            macos_precise_scroll_delta(&i.events)
        } else {
            i.smooth_scroll_delta
        };
        let zoom_delta = if trackpad_pan {
            1.0
        } else {
            native_pinch_delta.unwrap_or_else(|| i.zoom_delta())
        };
        let navigation_input = if trackpad_gesture.suppressed {
            NavigationInput::None
        } else {
            classify_navigation_input(zoom_delta, scroll_delta, trackpad_pan)
        };
        (
            i.pointer.hover_pos(),
            navigation_input,
            i.pointer.delta(),
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary),
        )
    });
    let (primary_down, primary_pressed, primary_released, middle_pressed, middle_released) = ui
        .input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.button_released(egui::PointerButton::Middle),
            )
        });
    let (command, space_down, alt, now) = ui.input(|i| {
        (
            i.modifiers.command || i.modifiers.ctrl,
            i.key_down(egui::Key::Space),
            i.modifiers.alt,
            i.time,
        )
    });
    let typing = ui.ctx().egui_wants_keyboard_input();
    if app.session.ui.wheel_zoom.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }

    // A double-click is reported on the second release. By then a zero-distance
    // box or axis zoom has already started on the second press, so handle the
    // click before the in-flight zoom completion path can consume that release.
    if dbl
        && let Some(p) = hover.filter(|p| rect.contains(*p))
        && !command
        && let Some((id, outer, plot)) = plot_under_cursor(app, ci, rect, p)
    {
        app.finish_pending_wheel_zoom(now, true);
        app.finish_pending_wheel_property(now, true);
        // A pan already moved the viewport and `cancel_interaction` has no Pan
        // arm, so cancelling would leave that move applied with no undo record
        // and make the reset's own record start from the panned view. Commit it
        // first: the pan and the reset then undo as the two edits they are.
        if matches!(app.interaction(), Interaction::Pan(_)) {
            commit_data_pan(app);
        } else if app.interaction().is_active() {
            app.cancel_interaction();
        }
        reset_plot_viewport(app, ci, id, outer, plot, p);
        return true;
    }

    // Every viewport zoom drag is owned here start-to-finish so the ambient
    // Alt+drag box gesture and the dedicated Browse Zoom tool share one path.
    let active_zoom = match &app.session.ui.interaction {
        Interaction::Zoom(d) => Some(*d),
        _ => None,
    };
    if let Some(drag) = active_zoom {
        if let Some(pp) = hover
            && let Interaction::Zoom(d) = &mut app.session.ui.interaction
        {
            d.current = [pp.x, pp.y];
        }
        if primary_released || !primary_down {
            if let Some(plot) = plot_inner_rect(app, drag.canvas, drag.object, rect)
                && let Interaction::Zoom(d) = app.take_interaction()
            {
                if d.axis == ZoomAxis::Box {
                    finish_zoom_drag(app, drag.canvas, drag.object, plot, d);
                } else {
                    finish_axis_zoom(app, drag.canvas, drag.object, plot, d);
                }
            } else {
                app.reset_interaction();
            }
        }
        ui.ctx().request_repaint();
        return true;
    }

    let panning = matches!(app.session.ui.interaction, Interaction::Pan(_));
    let pan_input = pan_input(ui);

    // Finish an in-flight data-pan even if the pointer has left the canvas.
    let Some(p) = hover.filter(|p| rect.contains(*p)) else {
        if panning && (middle_released || primary_released || !pan_input) {
            commit_data_pan(app);
            return true;
        }
        return panning;
    };

    let trackpad_target = trackpad_target(ui.ctx(), trackpad_gesture, ci, now, p, || {
        choose_trackpad_target(app, ci, rect, p, trackpad_gesture.board_target)
    });

    let data_target = if command {
        None
    } else {
        plot_under_cursor(app, ci, rect, p)
    };

    // Continue / end an in-flight data-pan; its target is locked at gesture start.
    if panning {
        let target = if let Interaction::Pan(drag) = &app.session.ui.interaction {
            Some((drag.canvas, drag.object))
        } else {
            None
        };
        if let Some((dci, did)) = target
            && pan_input
            && delta != Vec2::ZERO
            && let Some(plot) = plot_inner_rect(app, dci, did, rect)
        {
            app.session.viewport_mode = ViewportMode::Manual;
            apply_plot_pan(app, dci, did, plot, delta);
            ui.ctx().request_repaint();
        }
        if middle_released || primary_released || !pan_input {
            commit_data_pan(app);
        }
        return true;
    }

    if !typing {
        match navigation_input {
            NavigationInput::TrackpadPan(delta) => {
                match trackpad_target {
                    Some(TrackpadNavigationTarget::Plot { canvas, object }) => {
                        app.finish_pending_wheel_property(now, true);
                        if let Some(plot) = plot_inner_rect(app, canvas, object, rect) {
                            pan_plot_viewport(app, canvas, object, plot, delta, now);
                        }
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    Some(TrackpadNavigationTarget::BoardZoom) => {
                        let factor = swipe_zoom_factor(ui.ctx(), delta);
                        zoom_board_view(app, rect, p, factor);
                        mark_workspace_navigation(ui.ctx(), now);
                        ui.ctx().request_repaint();
                    }
                    Some(TrackpadNavigationTarget::Board) | None => {
                        pan_board_view(app, delta);
                        mark_workspace_navigation(ui.ctx(), now);
                        ui.ctx().request_repaint();
                    }
                }
                return true;
            }
            NavigationInput::Pinch(zoom_delta) => {
                let pinch_target = if command {
                    None
                } else {
                    selected_plot_body(app, ci, rect, p)
                };
                match pinch_target {
                    Some((id, outer, plot)) => {
                        app.finish_pending_wheel_property(now, true);
                        let scale = (1.0 / f64::from(zoom_delta)).clamp(0.2, 5.0);
                        app.session.viewport_mode = ViewportMode::Manual;
                        zoom_plot_viewport(
                            app,
                            ci,
                            id,
                            outer,
                            plot,
                            p,
                            scale,
                            (true, true),
                            now,
                            ui,
                        );
                    }
                    None => {
                        app.session.viewport_mode = ViewportMode::Manual;
                        zoom_board_view(app, rect, p, zoom_delta);
                        mark_workspace_navigation(ui.ctx(), now);
                        ui.ctx().request_repaint();
                    }
                }
                return true;
            }
            NavigationInput::WheelZoom(amount) => {
                let consumed = match data_target {
                    Some((id, outer, plot)) => {
                        if hit_zone(p, outer, plot) == HitZone::Plot
                            && (alt || supports_peak_height(app, ci, id))
                        {
                            if !supports_peak_height(app, ci, id) {
                                app.finish_pending_wheel_zoom(now, true);
                            }
                            adjust_plot_display(app, ci, id, plot, p, amount, now, ui)
                        } else {
                            app.finish_pending_wheel_property(now, true);
                            let Some(axes) = wheel_zoom_axes(app, ci, id, hit_zone(p, outer, plot))
                            else {
                                return false;
                            };
                            let scale =
                                f64::from((-amount * WHEEL_ZOOM_SPEED).exp()).clamp(0.2, 5.0);
                            app.session.viewport_mode = ViewportMode::Manual;
                            zoom_plot_viewport(app, ci, id, outer, plot, p, scale, axes, now, ui);
                            true
                        }
                    }
                    None => {
                        let factor = (amount * WHEEL_ZOOM_SPEED).exp();
                        app.session.viewport_mode = ViewportMode::Manual;
                        zoom_board_view(app, rect, p, factor);
                        mark_workspace_navigation(ui.ctx(), now);
                        ui.ctx().request_repaint();
                        true
                    }
                };
                return consumed;
            }
            NavigationInput::None => {}
        }
    }

    let pointer_navigation = pointer_navigation(app, ci, rect, p, ui);
    if primary_pressed && let Some(PointerNavigation::Zoom(id, axis)) = pointer_navigation {
        freeze_board_for_gesture(app);
        app.begin_interaction(Interaction::Zoom(ZoomDrag {
            canvas: ci,
            object: id,
            start: [p.x, p.y],
            current: [p.x, p.y],
            axis,
        }));
        return true;
    }

    let pan_started = middle_pressed || (space_down && primary_pressed);
    if pan_input {
        if pan_started
            && let Some(PointerNavigation::Pan(Some(id))) = pointer_navigation
            && let Some(before) = app.doc.canvases[ci]
                .object(id)
                .and_then(|object| object.plot())
                .map(|plot_object| plot_object.viewport.clone())
        {
            freeze_board_for_gesture(app);
            app.begin_interaction(Interaction::Pan(PanDrag {
                canvas: ci,
                object: id,
                before,
            }));
            return true;
        }
        if delta != Vec2::ZERO {
            pan_board_view(app, delta);
            mark_workspace_navigation(ui.ctx(), now);
        }
        ui.ctx().request_repaint();
        return true;
    }

    false
}

pub(crate) fn pan_board_view(app: &mut PlotiumApp, delta: Vec2) {
    app.session.viewport_mode = ViewportMode::Manual;
    let zoom = app.session.board.zoom.max(0.01);
    app.session.board.world_center[0] -= delta.x / zoom;
    app.session.board.world_center[1] -= delta.y / zoom;
}

pub(crate) fn zoom_board_view(app: &mut PlotiumApp, rect: egui::Rect, anchor: Pos2, factor: f32) {
    let old_zoom = app.session.board.zoom.max(0.01);
    let new_zoom = (old_zoom * factor).clamp(0.05, 8.0);
    let offset = anchor - rect.center();
    let world_x = app.session.board.world_center[0] + offset.x / old_zoom;
    let world_y = app.session.board.world_center[1] + offset.y / old_zoom;
    app.session.board.zoom = new_zoom;
    app.session.board.world_center = [world_x - offset.x / new_zoom, world_y - offset.y / new_zoom];
    app.session.viewport_mode = ViewportMode::Manual;
}

/// The pan gesture's undo bracket is owned by the caller (`handle_navigation`).
pub(crate) fn apply_plot_pan(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    plot: PlotRect,
    delta: Vec2,
) {
    let Some(plot_object) = app.doc.canvases[ci]
        .object_mut(object_id)
        .and_then(|object| object.plot_mut())
    else {
        return;
    };
    let fig = plot_object.figure();
    let x_sign = if fig.x.reversed { 1.0 } else { -1.0 };
    let y_sign = if fig.y.reversed { -1.0 } else { 1.0 };
    let x_fraction = x_sign * f64::from(delta.x) / f64::from(plot.width.max(1.0));
    let y_fraction = y_sign * f64::from(delta.y) / f64::from(plot.height.max(1.0));
    plot_object.pan_viewport(x_fraction, y_fraction);
    app.sync_linked_x_viewports(ci, object_id);
    app.mark_document_dirty();
}

fn update_pending_viewport_edit(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    now: f64,
) -> bool {
    let targets_another_plot = app
        .session
        .ui
        .wheel_zoom
        .as_ref()
        .is_some_and(|pending| pending.canvas != ci || pending.object != object_id);
    if targets_another_plot {
        app.finish_pending_wheel_zoom(now, true);
    }

    let Some(before) = app
        .doc
        .canvases
        .get(ci)
        .and_then(|canvas| canvas.object(object_id))
        .and_then(|object| object.plot())
        .map(|plot_object| plot_object.viewport.clone())
    else {
        return false;
    };
    if app.session.ui.wheel_zoom.is_none() {
        app.session.ui.wheel_zoom = Some(PendingViewportEdit {
            canvas: ci,
            object: object_id,
            before,
            last_input_time: now,
        });
    }
    if let Some(pending) = &mut app.session.ui.wheel_zoom {
        pending.last_input_time = now;
    }
    true
}

pub(crate) fn pan_plot_viewport(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    plot: PlotRect,
    delta: Vec2,
    now: f64,
) {
    if !update_pending_viewport_edit(app, ci, object_id, now) {
        return;
    }
    app.session.viewport_mode = ViewportMode::Manual;
    apply_plot_pan(app, ci, object_id, plot, delta);
}

pub(crate) fn commit_data_pan(app: &mut PlotiumApp) {
    if let Interaction::Pan(drag) = app.take_interaction()
        && let Some(object) = app.doc.canvases[drag.canvas]
            .object(drag.object)
            .and_then(|object| object.plot())
    {
        app.commit_object_viewport(
            drag.canvas,
            drag.object,
            drag.before,
            object.viewport.clone(),
        );
    }
}

pub(crate) fn finish_axis_zoom(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    plot: PlotRect,
    drag: ZoomDrag,
) {
    let a = clamp_to_plot(plot, pos(drag.start));
    let b = clamp_to_plot(plot, pos(drag.current));
    let Some(plot_object) = app.doc.canvases[ci]
        .object(object_id)
        .and_then(|object| object.plot())
    else {
        return;
    };
    let fig = plot_object.figure();
    let before = plot_object.viewport.clone();
    let (x, y) = match drag.axis {
        ZoomAxis::X => {
            if (a.x - b.x).abs() < SELECT_MIN_PX {
                return;
            }
            let range = AxisRange::new(
                from_screen_x(a.x, plot, &fig.x),
                from_screen_x(b.x, plot, &fig.x),
            );
            (Some(range), None)
        }
        ZoomAxis::Y => {
            if (a.y - b.y).abs() < SELECT_MIN_PX {
                return;
            }
            let range = AxisRange::new(
                from_screen_y(a.y, plot, &fig.y),
                from_screen_y(b.y, plot, &fig.y),
            );
            (None, Some(range))
        }
        ZoomAxis::Box => return,
    };
    let mut after = before.clone();
    after.select(fig, x, y);
    app.commit_object_viewport(ci, object_id, before, after);
    app.session.status = "Zoomed axis.".into();
}

pub(crate) fn reset_plot_viewport(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    outer_rect: EguiRect,
    plot: PlotRect,
    p: Pos2,
) {
    let Some(plot_object) = app.doc.canvases[ci]
        .object(object_id)
        .and_then(|object| object.plot())
    else {
        return;
    };
    let before = plot_object.viewport.clone();
    let mut after = before.clone();
    match hit_zone(p, outer_rect, plot) {
        HitZone::XAxis => after.reset_x(plot_object.figure()),
        HitZone::YAxis => after.reset_y(plot_object.figure()),
        HitZone::Plot => after.reset_all(),
        HitZone::None => return,
    }
    app.commit_object_viewport(ci, object_id, before, after);
}

/// The axes a plain wheel gesture addresses. A line plot is conventionally
/// navigated along its independent x coordinate; a raster-like field has two
/// spatial coordinates and navigates both. Axis strips always override that
/// body convention with the one axis they explicitly name.
fn wheel_zoom_axes(
    app: &PlotiumApp,
    canvas: usize,
    object: ObjectId,
    zone: HitZone,
) -> Option<(bool, bool)> {
    match zone {
        HitZone::XAxis => Some((true, false)),
        HitZone::YAxis => Some((false, true)),
        HitZone::Plot => {
            let two_dimensional = app.doc.canvases[canvas]
                .object(object)
                .and_then(|object| object.plot())
                .is_some_and(|plot| {
                    plot.binding.series.iter().any(|series| {
                        matches!(
                            &series.encoding,
                            plotium_figure::SeriesEncoding::Contour(_)
                                | plotium_figure::SeriesEncoding::Heatmap(_)
                                | plotium_figure::SeriesEncoding::Image(_)
                        )
                    })
                });
            Some((true, two_dimensional))
        }
        HitZone::None => None,
    }
}

/// Alt+wheel changes the unique display-sensitivity property exposed by the
/// hovered series. If there is no such property, a numeric 1D plot falls back
/// to y-scale intensity. Different eligible encodings are refused explicitly.
#[allow(clippy::too_many_arguments)]
fn adjust_plot_display(
    app: &mut PlotiumApp,
    canvas: usize,
    object: ObjectId,
    plot: PlotRect,
    pointer: Pos2,
    scroll_y: f32,
    now: f64,
    ui: &Ui,
) -> bool {
    use crate::ui::properties::discovery::{CanvasStepTarget, canvas_step_target};

    if app.doc.canvases[canvas]
        .object(object)
        .is_some_and(|object| object.locked)
    {
        app.session.status = crate::ui::properties::discovery::LOCKED_REASON.to_owned();
        return true;
    }

    match canvas_step_target(app, canvas, object) {
        CanvasStepTarget::Ambiguous { labels } => {
            app.finish_pending_wheel_property(now, true);
            app.session.status = format!(
                "Alt+scroll is ambiguous here ({}). Choose a layer in the Object inspector.",
                labels.join(" / ")
            );
            true
        }
        CanvasStepTarget::Unique {
            property,
            label,
            targets,
        } => {
            step_wheel_property(
                app, canvas, object, property, label, targets, scroll_y, now, ui,
            );
            true
        }
        CanvasStepTarget::None => {
            app.finish_pending_wheel_property(now, true);
            scale_line_intensity(app, canvas, object, plot, pointer, scroll_y, now, ui)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_wheel_property(
    app: &mut PlotiumApp,
    canvas: usize,
    object: ObjectId,
    property: plotium_core::properties::PropertyId,
    label: &'static str,
    targets: Vec<plotium_core::automation::TargetRef>,
    scroll_y: f32,
    now: f64,
    ui: &Ui,
) {
    let step_delta = ui
        .ctx()
        .options(|options| options.input_options.line_scroll_speed)
        .max(1.0);
    let changed_target = app
        .session
        .ui
        .wheel_property
        .as_ref()
        .is_some_and(|pending| {
            pending.canvas != canvas
                || pending.object != object
                || pending.property != property
                || pending.targets != targets
        });
    if changed_target {
        app.finish_pending_wheel_property(now, true);
    }
    if app.session.ui.wheel_property.is_none() {
        app.session.ui.wheel_property = Some(PendingWheelPropertyEdit {
            canvas,
            object,
            property,
            targets: targets.clone(),
            accumulator: 0.0,
            last_input_time: now,
            gesture_started: false,
        });
    }
    let pending = app.session.ui.wheel_property.as_mut().unwrap();
    pending.accumulator += scroll_y;
    pending.last_input_time = now;
    let steps = (pending.accumulator.abs() / step_delta).floor() as usize;
    if steps == 0 {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
        return;
    }
    let direction = if pending.accumulator > 0.0 {
        plotium_core::properties::PropertyStep::Lower
    } else {
        plotium_core::properties::PropertyStep::Raise
    };
    let signed_step = if pending.accumulator > 0.0 {
        step_delta
    } else {
        -step_delta
    };
    pending.accumulator -= signed_step * steps as f32;

    for _ in 0..steps.min(8) {
        match app.plan_property_step(property, &targets, direction) {
            Ok(commit) => {
                if !app
                    .session
                    .ui
                    .wheel_property
                    .as_ref()
                    .is_some_and(|pending| pending.gesture_started)
                {
                    app.begin_property_gesture(property);
                    if let Some(pending) = app.session.ui.wheel_property.as_mut() {
                        pending.gesture_started = true;
                    }
                }
                let applied = app.commit_property(commit);
                app.session.status = format!(
                    "Adjusted {label} on {applied} series. Alt+scroll controls display sensitivity."
                );
            }
            Err(error) => {
                app.session.status = format!("Could not adjust {label}: {error}");
                break;
            }
        }
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(200));
}

/// Zoom a plot's data viewport around the cursor on the requested axes.
/// Coalesces one stream of wheel or pinch events into one undo step.
#[allow(clippy::too_many_arguments)]
pub(crate) fn zoom_plot_viewport(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    _outer_rect: EguiRect,
    plot: PlotRect,
    p: Pos2,
    scale: f64,
    axes: (bool, bool),
    now: f64,
    ui: &Ui,
) {
    let (zoom_x, zoom_y) = axes;
    if !zoom_x && !zoom_y {
        return;
    }

    if !update_pending_viewport_edit(app, ci, object_id, now) {
        return;
    }

    let Some(plot_object) = app.doc.canvases[ci]
        .object_mut(object_id)
        .and_then(|object| object.plot_mut())
    else {
        return;
    };
    let fig = plot_object.figure();
    let x_anchor = zoom_x.then(|| from_screen_x(p.x, plot, &fig.x));
    let y_anchor = zoom_y.then(|| from_screen_y(p.y, plot, &fig.y));
    plot_object.zoom_viewport_around(x_anchor, y_anchor, scale);
    app.sync_linked_x_viewports(ci, object_id);
    app.mark_document_dirty();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(200));
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
