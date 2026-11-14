use super::*;
use plotium_core::layout::{PlotEdges, PlotInsets};

/// Distance (screen px) within which a dragged frame's edge snaps to a
/// neighbour's edge or to one gutter clear of it. Converted to world pt via the
/// live board zoom so the magnet feels the same at any zoom.
const FRAME_SNAP_TOL_PX: f32 = 8.0;

/// The snapped resting position for a frame being dragged to `candidate` (its
/// top-left, pt): edge/gutter magnetism against every other visible frame. An
/// axis with no nearby magnetic target stays exactly at the candidate position.
pub(crate) fn snap_dragged_frame(
    app: &PlotiumApp,
    frame: FrameRef,
    moving: &[plotium_core::state::BoardFrameId],
    candidate: [f32; 2],
    bypass: bool,
) -> [f32; 2] {
    if !app.settings.general.snap_enabled || bypass {
        return candidate;
    }
    let Some(r) = frame_board_rect(app, frame) else {
        return candidate;
    };
    let size = [r.right() - r.left, r.bottom() - r.top];
    let others: Vec<PlotRect> = board_frames(app)
        .into_iter()
        .filter(|&frame| board_frame_display(app, frame).shows_content())
        .filter(|&f| f != frame && board_frame_id(app, f).is_none_or(|id| !moving.contains(&id)))
        .filter_map(|f| frame_board_rect(app, f))
        .collect();
    let tol = FRAME_SNAP_TOL_PX / app.session.board.zoom.max(0.01);
    snap_frame_pos(candidate, size, &others, BOARD_GUTTER_PT, tol)
}

/// Snap a dragged frame (top-left `candidate`, world `size`) to the `others`
/// frames per axis: align to a neighbour's edge, or sit one `gutter` clear of it.
/// An axis with no neighbour within `tol` remains at its candidate coordinate.
fn snap_frame_pos(
    candidate: [f32; 2],
    size: [f32; 2],
    others: &[PlotRect],
    gutter: f32,
    tol: f32,
) -> [f32; 2] {
    let x = snap_edge(
        candidate[0],
        size[0],
        others.iter().map(|r| (r.left, r.right())),
        gutter,
        tol,
    );
    let y = snap_edge(
        candidate[1],
        size[1],
        others.iter().map(|r| (r.top, r.bottom())),
        gutter,
        tol,
    );
    [x.unwrap_or(candidate[0]), y.unwrap_or(candidate[1])]
}

/// The nearest snap target for a dragged frame's near edge `cand` (its extent
/// `extent` along this axis) against neighbour spans `lines` — each neighbour
/// offers aligning to either edge or sitting one `gutter` clear on either side.
/// `None` when nothing lands within `tol`.
fn snap_edge(
    cand: f32,
    extent: f32,
    lines: impl Iterator<Item = (f32, f32)>,
    gutter: f32,
    tol: f32,
) -> Option<f32> {
    let mut best = None;
    let mut best_d = tol;
    for (lo, hi) in lines {
        for target in [lo, hi - extent, hi + gutter, lo - gutter - extent] {
            let d = (target - cand).abs();
            if d < best_d {
                best_d = d;
                best = Some(target);
            }
        }
    }
    best
}

/// Snapping is skipped when disabled or Alt is held.
pub(crate) fn snap_object_frame(
    app: &PlotiumApp,
    ci: usize,
    drag: &ObjectDrag,
    candidate: ObjectFrame,
    keep_aspect: bool,
    ui: &Ui,
) -> (ObjectFrame, Vec<SnapGuide>) {
    let alt = ui.input(|i| i.modifiers.alt);
    if !app.settings.general.snap_enabled || alt {
        return (candidate, Vec::new());
    }
    let canvas = &app.doc.canvases[ci];
    let mut targets = SnapTargets::from_page(canvas.size_pt(), &canvas.layout);
    let moving =
        |id: ObjectId| id == drag.object || drag.others.iter().any(|(other, _)| *other == id);
    for object in &canvas.objects {
        if !moving(object.id)
            && object.visible
            && let Some(frame) = canvas.layout_frame(object.id)
        {
            targets.push_object(frame);
        }
    }
    let plots = other_plot_areas(canvas, moving);
    let insets = canvas
        .plot_area(drag.object)
        .zip(canvas.content_page_frame(drag.object))
        .map(|(plot, frame)| PlotInsets::of(frame, plot));
    let gesture = SnapGesture {
        kind: drag.kind,
        before: drag.before,
        keep_aspect,
        axes: drag.snap_axes,
        zoom: app.session.board.zoom.max(0.01),
    };
    snap_layout_frame(
        gesture,
        candidate,
        &targets,
        insets.map(|insets| (insets, &plots)),
    )
}

/// Plot-area edges of every visible plot on the page that `moving` excludes
/// as being dragged.
fn other_plot_areas(canvas: &CanvasDocument, moving: impl Fn(ObjectId) -> bool) -> PlotEdges {
    let mut edges = PlotEdges::default();
    for object in canvas.objects.iter().filter(|object| object.visible) {
        let hidden_panel = canvas
            .parent_panel(object.id)
            .and_then(|id| canvas.panel(id))
            .is_some_and(|panel| !panel.visible);
        if !moving(object.id)
            && !hidden_panel
            && let Some(area) = canvas.plot_area(object.id)
        {
            edges.push(area);
        }
    }
    edges
}

/// Snap a Panel child in Panel-local coordinates. Guides are translated back to
/// page coordinates because the overlay is deliberately rendered in page space.
pub(crate) fn snap_panel_content_frame(
    app: &PlotiumApp,
    ci: usize,
    panel_id: PanelId,
    drag: &ObjectDrag,
    candidate: ObjectFrame,
    ui: &Ui,
) -> (ObjectFrame, Vec<SnapGuide>) {
    let alt = ui.input(|i| i.modifiers.alt);
    if !app.settings.general.snap_enabled || alt {
        return (candidate, Vec::new());
    }
    let canvas = &app.doc.canvases[ci];
    let Some(panel) = canvas.panel(panel_id) else {
        return (candidate, Vec::new());
    };
    let mut targets = SnapTargets::from_page(
        [panel.frame.width, panel.frame.height],
        &plotium_core::layout::PageLayout::default(),
    );
    for &content in &panel.item_order {
        let moving = content == drag.object || drag.others.iter().any(|(id, _)| *id == content);
        if !moving
            && canvas.object(content).is_some_and(|item| item.visible)
            && let Some(frame) = canvas.object(content).map(|item| item.frame)
        {
            targets.push_object(frame);
        }
    }
    // Panel content always resizes aspect-locked.
    let gesture = SnapGesture {
        kind: drag.kind,
        before: drag.before,
        keep_aspect: true,
        axes: drag.snap_axes,
        zoom: app.session.board.zoom.max(0.01),
    };
    let (frame, guides) = snap_layout_frame(gesture, candidate, &targets, None);
    let guides = guides
        .into_iter()
        .map(|mut guide| {
            guide.pos += if guide.vertical {
                panel.frame.x
            } else {
                panel.frame.y
            };
            guide
        })
        .collect();
    (frame, guides)
}

/// Snap a moving or resizing Panel to the page edges, centre lines and layout
/// cells plus every other visible Panel's edges and centres. Panels moving with
/// it as a group are not targets. Snapping is skipped when disabled or `bypass`
/// (Alt) is held.
pub(crate) fn snap_panel_frame(
    app: &PlotiumApp,
    ci: usize,
    drag: &PanelDrag,
    candidate: ObjectFrame,
    bypass: bool,
) -> (ObjectFrame, Vec<SnapGuide>) {
    if !app.settings.general.snap_enabled || bypass {
        return (candidate, Vec::new());
    }
    let Some(canvas) = app.doc.canvases.get(ci) else {
        return (candidate, Vec::new());
    };
    let mut targets = SnapTargets::from_page(canvas.size_pt(), &canvas.layout);
    let moving_panel =
        |id: PanelId| id == drag.panel || drag.others.iter().any(|(other, _)| *other == id);
    for panel in &canvas.panels {
        if !moving_panel(panel.id) && panel.visible {
            targets.push_object(panel.frame);
        }
    }
    let plots = other_plot_areas(canvas, |id| {
        canvas.parent_panel(id).is_some_and(moving_panel)
    });
    let insets = canvas
        .panel_plot_area(drag.panel)
        .zip(canvas.panel(drag.panel))
        .map(|(plot, panel)| PlotInsets::of(panel.frame, plot));
    let gesture = SnapGesture {
        kind: drag.kind,
        before: drag.before,
        keep_aspect: false,
        axes: drag.snap_axes,
        zoom: app.session.board.zoom.max(0.01),
    };
    snap_layout_frame(
        gesture,
        candidate,
        &targets,
        insets.map(|insets| (insets, &plots)),
    )
}

#[derive(Clone, Copy)]
struct SnapGesture {
    kind: ObjectDragKind,
    before: ObjectFrame,
    keep_aspect: bool,
    axes: [bool; 2],
    zoom: f32,
}

fn snap_layout_frame(
    gesture: SnapGesture,
    candidate: ObjectFrame,
    targets: &SnapTargets,
    plot: layout::PlotSnap,
) -> (ObjectFrame, Vec<SnapGuide>) {
    let threshold = SNAP_PX / gesture.zoom;
    // A target below the drag floor would pull the frame to it. Plot and Panel
    // minimums apply after snapping, and clear the guides when they do.
    let min_size = if gesture.keep_aspect {
        plotium_core::state::aspect_locked_min_size(gesture.before, MIN_OBJECT_SIZE_PT)
    } else {
        resize_min_size(gesture.before, false)
    };
    match gesture.kind {
        ObjectDragKind::Move => layout::snap_move_with_plot(candidate, targets, plot, threshold),
        ObjectDragKind::Resize(handle) => layout::snap_resize_with_plot(
            candidate,
            dragged_edges(handle, gesture.axes),
            targets,
            plot,
            threshold,
            min_size,
        ),
    }
}

/// Records the axes along which the pointer has left the dead-zone, in screen px.
pub(crate) fn latch_snap_axes(axes: &mut [bool; 2], travel: [f32; 2]) {
    axes[0] |= travel[0].abs() > DRAG_START_PX;
    axes[1] |= travel[1].abs() > DRAG_START_PX;
}

/// The handle's edges along `axes`. A resize is live from the press, so an edge
/// near a target would otherwise snap, and with a locked aspect rescale the
/// object, without the pointer travelling that way.
fn dragged_edges(handle: ResizeHandle, axes: [bool; 2]) -> MovableEdges {
    let edges = movable_edges(handle);
    MovableEdges {
        left: edges.left && axes[0],
        right: edges.right && axes[0],
        top: edges.top && axes[1],
        bottom: edges.bottom && axes[1],
    }
}

/// Drops the guides of an axis whose edges a later rescale moved off the snap.
pub(crate) fn retain_landed_guides(
    guides: &mut Vec<SnapGuide>,
    snapped: ObjectFrame,
    frame: ObjectFrame,
) {
    const ROUNDING_PT: f32 = 1e-3;
    let same = |a: f32, b: f32| (a - b).abs() <= ROUNDING_PT;
    let held_x = same(snapped.x, frame.x) && same(snapped.width, frame.width);
    let held_y = same(snapped.y, frame.y) && same(snapped.height, frame.height);
    guides.retain(|guide| if guide.vertical { held_x } else { held_y });
}

pub(crate) fn movable_edges(handle: ResizeHandle) -> MovableEdges {
    let (left, right) = (
        matches!(handle, ResizeHandle::TopLeft | ResizeHandle::BottomLeft),
        matches!(handle, ResizeHandle::TopRight | ResizeHandle::BottomRight),
    );
    let (top, bottom) = (
        matches!(handle, ResizeHandle::TopLeft | ResizeHandle::TopRight),
        matches!(handle, ResizeHandle::BottomLeft | ResizeHandle::BottomRight),
    );
    MovableEdges {
        left,
        right,
        top,
        bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZOOM: f32 = 1.0;

    fn resize(
        before: ObjectFrame,
        candidate: ObjectFrame,
        keep_aspect: bool,
        ys: &[f32],
    ) -> (ObjectFrame, Vec<SnapGuide>) {
        let mut axes = [false; 2];
        latch_snap_axes(
            &mut axes,
            [
                candidate.width - before.width,
                candidate.height - before.height,
            ],
        );
        let gesture = SnapGesture {
            kind: ObjectDragKind::Resize(ResizeHandle::BottomRight),
            before,
            keep_aspect,
            axes,
            zoom: ZOOM,
        };
        let mut targets = SnapTargets::default();
        targets.ys.extend_from_slice(ys);
        snap_layout_frame(gesture, candidate, &targets, None)
    }

    #[test]
    fn pressing_a_handle_snaps_nothing() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 20.0);
        for keep_aspect in [false, true] {
            let (frame, guides) = resize(before, before, keep_aspect, &[24.0]);
            assert_eq!(frame, before);
            assert!(guides.is_empty());
        }
    }

    #[test]
    fn an_axis_the_drag_has_not_moved_does_not_snap() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 20.0);
        let along_x = ObjectFrame::new(0.0, 0.0, 130.0, 20.0);
        assert_eq!(resize(before, along_x, false, &[24.0]).0, along_x);
    }

    #[test]
    fn the_moved_axis_snaps_as_before() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 80.0);
        let candidate = ObjectFrame::new(0.0, 0.0, 100.0, 96.0);
        let (frame, guides) = resize(before, candidate, false, &[100.0]);
        assert_eq!(frame, ObjectFrame::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(guides.len(), 1);
    }

    #[test]
    fn a_large_object_skips_a_snap_below_the_minimum() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 80.0);
        let candidate = ObjectFrame::new(0.0, 0.0, 100.0, 26.0);
        assert_eq!(resize(before, candidate, false, &[20.0]).0, candidate);
    }

    #[test]
    fn a_thin_edge_snaps_below_the_object_minimum() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 15.0);
        let candidate = ObjectFrame::new(0.0, 0.0, 100.0, 21.0);
        let (frame, guides) = resize(before, candidate, false, &[18.0]);
        assert_eq!(frame.height, 18.0);
        assert_eq!(guides.len(), 1);
    }

    #[test]
    fn an_aspect_locked_shrink_snaps_below_the_object_minimum() {
        let kind = ObjectDragKind::Resize(ResizeHandle::BottomRight);
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 60.0);
        let vertical = ObjectFrame::new(0.0, 0.0, 100.0, 18.0);
        let (snapped, mut guides) = resize(before, vertical, true, &[20.0]);
        let frame = preserve_aspect_frame(before, snapped, kind);
        retain_landed_guides(&mut guides, snapped, frame);
        assert_eq!(frame.height, 20.0);
        assert_eq!(guides.len(), 1);
        assert_eq!(resize(before, vertical, false, &[20.0]).0, vertical);
    }

    #[test]
    fn an_aspect_locked_resize_skips_a_target_below_its_floor() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 60.0);
        let vertical = ObjectFrame::new(0.0, 0.0, 100.0, 18.0);
        let (snapped, guides) = resize(before, vertical, true, &[12.0]);
        assert_eq!(snapped, vertical);
        assert!(guides.is_empty());
    }

    #[test]
    fn a_guide_the_aspect_rescale_leaves_is_dropped() {
        let kind = ObjectDragKind::Resize(ResizeHandle::BottomRight);
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 60.0);
        let diagonal = ObjectFrame::new(0.0, 0.0, 30.0, 18.0);
        let (snapped, mut guides) = resize(before, diagonal, true, &[20.0]);
        assert_eq!(guides.len(), 1);
        let frame = preserve_aspect_frame(before, snapped, kind);
        retain_landed_guides(&mut guides, snapped, frame);
        assert!((frame.height - 18.0).abs() < 1e-3);
        assert!(guides.is_empty());
    }

    #[test]
    fn an_axis_stays_on_after_the_pointer_returns() {
        let mut axes = [false; 2];
        latch_snap_axes(&mut axes, [4.0, 5.0]);
        assert_eq!(axes, [false, false]);
        latch_snap_axes(&mut axes, [-30.0, 0.0]);
        latch_snap_axes(&mut axes, [3.0, 0.0]);
        assert_eq!(axes, [true, false]);

        let edges = dragged_edges(ResizeHandle::TopRight, axes);
        assert!(edges.right && !edges.top);
        let edges = dragged_edges(ResizeHandle::BottomLeft, [false, true]);
        assert!(edges.bottom && !edges.left);
    }

    #[test]
    fn free_placement_keeps_the_candidate_without_a_magnetic_target() {
        let others = [PlotRect::new(0.0, 0.0, 100.0, 80.0)];
        let size = [100.0, 80.0];
        let far = [1000.0 + 5.0, 700.0 + 5.0];
        assert_eq!(
            snap_frame_pos(far, size, &others, BOARD_GUTTER_PT, 8.0),
            far
        );
    }

    #[test]
    fn magnetic_edges_and_gutters_snap_within_tolerance() {
        let others = [PlotRect::new(0.0, 0.0, 100.0, 80.0)];
        let size = [100.0, 80.0];
        let near = [100.0 + BOARD_GUTTER_PT + 3.0, 2.0];
        assert_eq!(
            snap_frame_pos(near, size, &others, BOARD_GUTTER_PT, 8.0),
            [100.0 + BOARD_GUTTER_PT, 0.0]
        );
    }

    fn app_with_two_pages() -> PlotiumApp {
        let mut app = PlotiumApp::new();
        for x in [0.0, 500.0] {
            let mut page = CanvasDocument::new("p".to_owned(), [100.0, 80.0]);
            page.board_pos = [x, 0.0];
            app.doc.canvases.push(page);
        }
        app.session.board.zoom = 1.0;
        app
    }

    #[test]
    fn frame_snapping_respects_the_general_preference() {
        let mut app = app_with_two_pages();
        let width = app.doc.canvases[0].board_rect_pt().width;
        let candidate = [width + BOARD_GUTTER_PT + 2.0, 3.0];
        app.settings.general.snap_enabled = false;
        assert_eq!(
            snap_dragged_frame(&app, FrameRef::Page(1), &[], candidate, false),
            candidate
        );
    }

    #[test]
    fn alt_bypasses_frame_snapping_for_the_current_drag() {
        let app = app_with_two_pages();
        let width = app.doc.canvases[0].board_rect_pt().width;
        let candidate = [width + BOARD_GUTTER_PT + 2.0, 3.0];
        assert_eq!(
            snap_dragged_frame(&app, FrameRef::Page(1), &[], candidate, true),
            candidate
        );
    }

    #[test]
    fn group_drag_does_not_snap_to_another_moving_page() {
        let mut app = app_with_two_pages();
        let width = app.doc.canvases[0].board_rect_pt().width;
        app.doc.canvases[1].board_pos = [width, 0.0];
        let moving = app
            .doc
            .canvases
            .iter()
            .map(|canvas| plotium_core::state::BoardFrameId::Page(canvas.resource_id))
            .collect::<Vec<_>>();
        let candidate = [3.0, 3.0];

        assert_eq!(
            snap_dragged_frame(&app, FrameRef::Page(0), &moving, candidate, false),
            candidate
        );
    }

    fn app_with_panels(frames: &[ObjectFrame]) -> (PlotiumApp, Vec<PanelId>) {
        let mut app = PlotiumApp::new();
        let mut page = CanvasDocument::new("p".to_owned(), [600.0, 400.0]);
        let ids = frames
            .iter()
            .map(|&frame| page.create_panel("panel".to_owned(), frame))
            .collect();
        app.doc.canvases.push(page);
        app.session.board.zoom = 1.0;
        (app, ids)
    }

    fn panel_drag(app: &PlotiumApp, panel: PanelId, kind: ObjectDragKind) -> PanelDrag {
        let before = app.doc.canvases[0].panel(panel).unwrap().frame;
        PanelDrag {
            canvas: 0,
            panel,
            kind,
            before,
            others: Vec::new(),
            children: Vec::new(),
            start_pointer: [before.x, before.y],
            start_pointer_screen: [0.0, 0.0],
            snap_axes: [true; 2],
            active: true,
            detached_since: None,
        }
    }

    #[test]
    fn panel_move_snaps_to_a_sibling_panel_edge() {
        let (app, ids) = app_with_panels(&[
            ObjectFrame::new(20.0, 20.0, 100.0, 80.0),
            ObjectFrame::new(350.0, 250.0, 100.0, 80.0),
        ]);
        let drag = panel_drag(&app, ids[1], ObjectDragKind::Move);
        let candidate = ObjectFrame::new(123.0, 250.0, 100.0, 80.0);

        let (frame, guides) = snap_panel_frame(&app, 0, &drag, candidate, false);

        assert_eq!(frame, ObjectFrame::new(120.0, 250.0, 100.0, 80.0));
        assert_eq!(
            guides,
            vec![SnapGuide {
                vertical: true,
                pos: 120.0
            }]
        );
    }

    #[test]
    fn panel_resize_snaps_its_moving_edge_to_the_page_edge() {
        let (app, ids) = app_with_panels(&[ObjectFrame::new(350.0, 250.0, 100.0, 80.0)]);
        let drag = panel_drag(
            &app,
            ids[0],
            ObjectDragKind::Resize(ResizeHandle::BottomRight),
        );
        let page_width = app.doc.canvases[0].size_pt()[0];
        let candidate = ObjectFrame::new(350.0, 250.0, page_width - 353.0, 80.0);

        let (frame, guides) = snap_panel_frame(&app, 0, &drag, candidate, false);

        assert_eq!(
            frame,
            ObjectFrame::new(350.0, 250.0, page_width - 350.0, 80.0)
        );
        assert_eq!(
            guides,
            vec![SnapGuide {
                vertical: true,
                pos: page_width
            }]
        );
    }

    #[test]
    fn pressing_a_panel_handle_snaps_nothing() {
        let frame = ObjectFrame::new(40.0, 30.0, 100.0, 20.0);
        let (mut app, ids) = app_with_panels(&[frame]);
        app.settings.general.snap_enabled = true;
        let [width, height] = app.doc.canvases[0].size_pt();
        let close = ObjectFrame::new(width - 103.0, height - 23.0, frame.width, frame.height);
        app.doc.canvases[0].panel_mut(ids[0]).unwrap().frame = close;
        let kind = ObjectDragKind::Resize(ResizeHandle::BottomRight);
        let mut drag = panel_drag(&app, ids[0], kind);
        drag.snap_axes = [false; 2];
        let (snapped, guides) = snap_panel_frame(&app, 0, &drag, close, false);
        assert_eq!(snapped, close);
        assert!(guides.is_empty());
    }

    #[test]
    fn panel_snapping_ignores_group_members_and_alt() {
        let (app, ids) = app_with_panels(&[
            ObjectFrame::new(20.0, 20.0, 100.0, 80.0),
            ObjectFrame::new(350.0, 250.0, 100.0, 80.0),
        ]);
        let candidate = ObjectFrame::new(123.0, 250.0, 100.0, 80.0);
        let mut drag = panel_drag(&app, ids[1], ObjectDragKind::Move);

        let (frame, guides) = snap_panel_frame(&app, 0, &drag, candidate, true);
        assert_eq!((frame, guides.len()), (candidate, 0));

        drag.others = vec![(ids[0], app.doc.canvases[0].panel(ids[0]).unwrap().frame)];
        let (frame, guides) = snap_panel_frame(&app, 0, &drag, candidate, false);
        assert_eq!((frame, guides.len()), (candidate, 0));
    }

    /// A page with two plot panels in one column whose y tick labels differ in
    /// width, so frames that line up hold plot areas that do not.
    fn app_with_plot_panels() -> (PlotiumApp, [PanelId; 2]) {
        use plotium_core::state::{Dataset, ElectrophysiologyDataset};
        let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
        let recording = plotium_io::ElectrophysiologyData {
            abf_version: "2.9.0.0".to_owned(),
            sample_rate_hz: 10_000.0,
            channels: vec![plotium_io::RecordedChannel {
                name: "Current".to_owned(),
                unit: plotium_io::ElectricalUnit::from_symbol("pA"),
            }],
            sweeps: vec![plotium_io::Sweep {
                start_time_s: 0.0,
                channels: vec![vec![0.0, -1.0, 0.5]],
                commands: Vec::new(),
            }],
            protocol: None,
            source: "snap.abf".to_owned(),
            import_warnings: Vec::new(),
        };
        app.doc.datasets.push(Dataset::Electrophysiology(Box::new(
            ElectrophysiologyDataset::load(recording),
        )));
        app.doc
            .canvases
            .push(CanvasDocument::new("p".to_owned(), [600.0, 600.0]));
        app.session.active_canvas = Some(0);
        app.session.board.zoom = 1.0;
        let mut panels = Vec::new();
        for (y, wide) in [(20.0, false), (260.0, true)] {
            let id = app.doc.canvases[0].allocate_object_id();
            let plot = app.build_plot_object(
                0,
                ObjectFrame::new(20.0, y, 300.0, 200.0),
                id,
                "plot".to_owned(),
            );
            app.doc.canvases[0].objects.push(plot);
            panels.push(app.doc.canvases[0].create_panel_for_plot(id).unwrap());
            if wide {
                let mut overrides = app.doc.canvases[0]
                    .object(id)
                    .and_then(|object| object.plot())
                    .unwrap()
                    .axis_overrides
                    .clone();
                overrides.y_range = Some(AxisRange::new(-100_000.0, 100_000.0));
                app.set_axis_overrides_value(0, id, &overrides);
            }
        }
        (app, [panels[0], panels[1]])
    }

    #[test]
    fn panel_move_lines_its_plot_area_up_with_a_sibling_plot() {
        let (app, [upper, lower]) = app_with_plot_panels();
        let page = &app.doc.canvases[0];
        let upper_left = page.panel_plot_area(upper).unwrap().x;
        let lower_inset = page.panel_plot_area(lower).unwrap().x - 20.0;
        assert!(
            (lower_inset - (upper_left - 20.0)).abs() > 3.0,
            "tick labels differ"
        );
        let drag = panel_drag(&app, lower, ObjectDragKind::Move);
        // Nearer lining the plot areas up than lining the frames up.
        let x = upper_left - lower_inset + 1.5;
        let candidate = ObjectFrame::new(x, 260.0, 300.0, 200.0);

        let (frame, guides) = snap_panel_frame(&app, 0, &drag, candidate, false);

        assert!(
            (frame.x + lower_inset - upper_left).abs() < 1e-3,
            "{frame:?}"
        );
        assert!(
            guides
                .iter()
                .any(|guide| guide.vertical && (guide.pos - upper_left).abs() < 1e-3)
        );
    }
}
