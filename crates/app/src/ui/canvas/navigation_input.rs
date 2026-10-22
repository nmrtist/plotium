use super::{HitZone, PlotiumApp, hit_zone, plot_under_cursor};
use egui::{Pos2, Rect as EguiRect, Vec2};
use plotium_core::state::ObjectId;
use plotium_render::Rect as PlotRect;

/// winit reports the momentum phase of a fling as a new `Start`; one that
/// follows an `End` this closely, or in the next pass, at the same pointer
/// continues that gesture.
const MOMENTUM_CONTINUATION_S: f64 = 0.1;
const MOMENTUM_POINTER_SLOP_PX: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum NavigationInput {
    None,
    TrackpadPan(Vec2),
    WheelZoom(f32),
    Pinch(f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MacosTrackpadGesture {
    pub(super) active: bool,
    pub(super) suppressed: bool,
    pub(super) board_target: bool,
    pub(super) started: bool,
    /// Counts `Start` phases, so a stored target can tell which gesture it belongs to.
    pub(super) serial: u64,
    /// An `End` and the next `Start` arrived in the same frame.
    pub(super) restarted: bool,
    pub(super) finished: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TrackpadNavigationTarget {
    #[default]
    Board,
    BoardZoom,
    Plot {
        canvas: usize,
        object: ObjectId,
    },
}

/// A swipe's target, kept after `End` so a momentum `Start` can resume it.
#[derive(Clone, Copy, Default)]
struct TrackpadTargetState {
    target: TrackpadNavigationTarget,
    serial: u64,
    ended: Option<GestureEnd>,
}

#[derive(Clone, Copy)]
struct GestureEnd {
    time: f64,
    pass: u64,
    pointer: Pos2,
}

fn trackpad_target_id() -> egui::Id {
    egui::Id::new("plotium.trackpad_navigation_target")
}

/// The plot under `p` when it is the selected plot and `p` is on its data body.
/// Only there do trackpad gestures navigate data instead of the board.
pub(crate) fn selected_plot_body(
    app: &PlotiumApp,
    ci: usize,
    rect: egui::Rect,
    p: Pos2,
) -> Option<(ObjectId, EguiRect, PlotRect)> {
    if app.session.active_canvas != Some(ci) {
        return None;
    }
    let (object, outer, plot) = plot_under_cursor(app, ci, rect, p)?;
    let selection = app.session.ui.selection.objects();
    let group = app.doc.canvases.get(ci)?.group_members(object);
    let selected =
        selection.first() == Some(&object) && selection.iter().all(|id| group.contains(id));
    (selected && hit_zone(p, outer, plot) == HitZone::Plot).then_some((object, outer, plot))
}

/// Lock a swipe's target at its start: Cmd/Ctrl zooms the board, the selected
/// plot's body pans its data, anything else pans the board. A momentum start
/// keeps the previous target.
pub(super) fn trackpad_target(
    ctx: &egui::Context,
    gesture: MacosTrackpadGesture,
    ci: usize,
    now: f64,
    p: Pos2,
    choose: impl FnOnce() -> TrackpadNavigationTarget,
) -> Option<TrackpadNavigationTarget> {
    if !gesture.active {
        return None;
    }
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| {
        let stored = data.get_temp::<TrackpadTargetState>(trackpad_target_id());
        let continued = stored.filter(|state| {
            let other_canvas = matches!(
                state.target,
                TrackpadNavigationTarget::Plot { canvas, .. } if canvas != ci
            );
            let next_gesture = gesture.started && state.serial + 1 == gesture.serial;
            !other_canvas
                && match state.ended {
                    None if gesture.started => next_gesture && gesture.restarted,
                    None => state.serial == gesture.serial,
                    Some(end) => {
                        next_gesture
                            && end.pointer.distance(p) <= MOMENTUM_POINTER_SLOP_PX
                            && (now - end.time <= MOMENTUM_CONTINUATION_S || pass == end.pass + 1)
                    }
                }
        });
        let target = continued.map_or_else(choose, |state| state.target);
        data.insert_temp(
            trackpad_target_id(),
            TrackpadTargetState {
                target,
                serial: gesture.serial,
                ended: gesture.finished.then_some(GestureEnd {
                    time: now,
                    pass,
                    pointer: p,
                }),
            },
        );
        Some(target)
    })
}

pub(super) fn choose_trackpad_target(
    app: &PlotiumApp,
    ci: usize,
    rect: egui::Rect,
    p: Pos2,
    board_zoom: bool,
) -> TrackpadNavigationTarget {
    if board_zoom {
        TrackpadNavigationTarget::BoardZoom
    } else if let Some((object, _, _)) = selected_plot_body(app, ci, rect, p) {
        TrackpadNavigationTarget::Plot { canvas: ci, object }
    } else {
        TrackpadNavigationTarget::Board
    }
}

/// Board zoom factor for a Cmd+swipe, matching egui's Cmd+wheel conversion.
pub(super) fn swipe_zoom_factor(ctx: &egui::Context, delta: Vec2) -> f32 {
    let speed = ctx.options(|options| options.input_options.scroll_zoom_speed);
    (speed * (delta.x + delta.y)).exp()
}

/// Remember whether the latest wheel or pinch input came from a macOS trackpad
/// gesture, so the hover hint can describe the device in hand.
pub(super) fn note_wheel_device(ctx: &egui::Context, events: &[egui::Event], trackpad: bool) {
    let pinch = cfg!(target_os = "macos")
        && events
            .iter()
            .any(|event| matches!(event, egui::Event::Zoom(_)));
    let wheel = events
        .iter()
        .any(|event| matches!(event, egui::Event::MouseWheel { .. }));
    if pinch || wheel {
        ctx.data_mut(|data| data.insert_temp(wheel_device_id(), pinch || trackpad));
    }
}

pub(crate) fn last_wheel_was_trackpad(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(wheel_device_id()))
        .unwrap_or(false)
}

fn wheel_device_id() -> egui::Id {
    egui::Id::new("plotium.last_wheel_trackpad")
}

pub(super) fn classify_navigation_input(
    zoom_delta: f32,
    scroll_delta: Vec2,
    precise_scroll: bool,
) -> NavigationInput {
    if (zoom_delta - 1.0).abs() > 0.001 {
        NavigationInput::Pinch(zoom_delta)
    } else if precise_scroll && scroll_delta != Vec2::ZERO {
        NavigationInput::TrackpadPan(scroll_delta)
    } else if scroll_delta.y != 0.0 {
        NavigationInput::WheelZoom(scroll_delta.y)
    } else {
        NavigationInput::None
    }
}

pub(super) fn macos_trackpad_gesture(
    ctx: &egui::Context,
    events: &[egui::Event],
    pointer_owned: bool,
) -> MacosTrackpadGesture {
    #[cfg(target_os = "macos")]
    {
        #[derive(Clone, Copy, Default)]
        struct StoredGesture {
            active: bool,
            suppressed: bool,
            board_target: bool,
        }

        let state_id = egui::Id::new("plotium.macos_trackpad_gesture");
        ctx.data_mut(|data| {
            let serial_id = egui::Id::new("plotium.macos_trackpad_gesture_serial");
            let mut serial = data.get_temp::<u64>(serial_id).unwrap_or_default();
            let mut stored = data.get_temp::<StoredGesture>(state_id).unwrap_or_default();
            let mut frame_target = stored.active.then_some(stored.board_target);
            let mut frame_suppressed = stored.suppressed;
            let mut started = false;
            let mut restarted = false;
            let mut finished = false;
            for event in events {
                let egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase,
                    modifiers,
                    ..
                } = event
                else {
                    continue;
                };
                match phase {
                    egui::TouchPhase::Start => {
                        started = pointer_owned;
                        serial += 1;
                        restarted = finished;
                        finished = false;
                        stored.active = pointer_owned;
                        stored.suppressed = !pointer_owned;
                        stored.board_target = modifiers.command || modifiers.ctrl;
                        frame_target = stored.active.then_some(stored.board_target);
                        frame_suppressed = stored.suppressed;
                    }
                    egui::TouchPhase::Move => {
                        if stored.active {
                            frame_target = Some(stored.board_target);
                        }
                        frame_suppressed = stored.suppressed;
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        finished = true;
                        if stored.active {
                            frame_target = Some(stored.board_target);
                        }
                        frame_suppressed = stored.suppressed;
                        stored = StoredGesture::default();
                    }
                }
            }
            data.insert_temp(serial_id, serial);
            if stored.active || stored.suppressed {
                data.insert_temp(state_id, stored);
            } else {
                data.remove_temp::<StoredGesture>(state_id);
            }
            MacosTrackpadGesture {
                active: frame_target.is_some(),
                suppressed: frame_suppressed,
                board_target: frame_target.unwrap_or(false),
                started,
                serial,
                restarted: restarted && started,
                finished,
            }
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, events, pointer_owned);
        MacosTrackpadGesture::default()
    }
}

pub(crate) fn sync_macos_trackpad_gesture(
    ctx: &egui::Context,
    events: &[egui::Event],
    pointer_owned: bool,
) {
    if pointer_owned {
        return;
    }
    #[cfg(target_os = "macos")]
    if events.iter().any(|event| {
        matches!(
            event,
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                ..
            }
        )
    }) {
        let gesture = macos_trackpad_gesture(ctx, events, false);
        if gesture.finished {
            ctx.data_mut(|data| data.remove_temp::<TrackpadTargetState>(trackpad_target_id()));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (ctx, events);
}

pub(super) fn macos_precise_scroll_delta(events: &[egui::Event]) -> Vec2 {
    #[cfg(target_os = "macos")]
    {
        events
            .iter()
            .filter_map(|event| match event {
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    ..
                } => Some(*delta),
                _ => None,
            })
            .fold(Vec2::ZERO, |sum, delta| sum + delta)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = events;
        Vec2::ZERO
    }
}
