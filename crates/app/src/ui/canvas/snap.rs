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
    ui: &Ui,
) -> (ObjectFrame, Vec<SnapGuide>) {
    let alt = ui.input(|i| i.modifiers.alt);
    if !app.settings.general.snap_enabled || alt {
        return (candidate, Vec::new());
    }
    let canvas = &app.doc.canvases[ci];
    let zoom = app.session.board.zoom.max(0.01);
    let threshold = SNAP_PX / zoom;
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
    snap_layout_frame(
        drag.kind,
        candidate,
        &targets,
        insets.map(|insets| (insets, &plots)),
        threshold,
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
    let zoom = app.session.board.zoom.max(0.01);
    let threshold = SNAP_PX / zoom;
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
    let (frame, guides) = snap_layout_frame(drag.kind, candidate, &targets, None, threshold);
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
    let threshold = SNAP_PX / app.session.board.zoom.max(0.01);
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
    snap_layout_frame(
        drag.kind,
        candidate,
        &targets,
        insets.map(|insets| (insets, &plots)),
        threshold,
    )
}

fn snap_layout_frame(
    kind: ObjectDragKind,
    candidate: ObjectFrame,
    targets: &SnapTargets,
    plot: layout::PlotSnap,
    threshold: f32,
) -> (ObjectFrame, Vec<SnapGuide>) {
    match kind {
        ObjectDragKind::Move => layout::snap_move_with_plot(candidate, targets, plot, threshold),
        ObjectDragKind::Resize(handle) => layout::snap_resize_with_plot(
            candidate,
            movable_edges(handle),
            targets,
            plot,
            threshold,
            MIN_OBJECT_SIZE_PT,
        ),
    }
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
