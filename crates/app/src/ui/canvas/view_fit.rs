//! Keyboard data-viewport fits. `H` fits the intensity axis to the data
//! visible inside the current x window (the NMR convention for a vertical
//! fit); `F` over a plot's data area fits both axes. Both are the keyboard
//! form of the double-click viewport resets in `navigation.rs` and commit
//! through the same undoable viewport action.

use super::*;

const NAVIGATION_RECT_ID: &str = "plotium.canvas.navigation_rect";

/// Which data-viewport axes a keyboard fit resets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlotFitAxes {
    /// Fit the y range to the data visible in the current x window and
    /// re-enable automatic y scaling; the x window stays put.
    Y,
    /// Fit both axes to the full data range.
    Both,
}

/// Publish the board rectangle canvas navigation ran against this frame, so
/// keyboard commands can hit-test the pointer without re-deriving layout.
pub(crate) fn store_navigation_rect(ctx: &egui::Context, rect: EguiRect) {
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new(NAVIGATION_RECT_ID), rect);
    });
}

fn navigation_rect(ctx: &egui::Context) -> Option<EguiRect> {
    ctx.data(|data| data.get_temp::<EguiRect>(egui::Id::new(NAVIGATION_RECT_ID)))
}

/// The pointer position, unless a floating task card sits under it — over a
/// card the pointer does not address the plot below (mirrors
/// `task_card::pointer_allows_canvas` for gesture dispatch).
fn uncovered_pointer(app: &PlotiumApp, ctx: &egui::Context) -> Option<Pos2> {
    let p = ctx.input(|input| input.pointer.hover_pos())?;
    let covered = crate::ui::tools::task_card::visible_area_id(app)
        .and_then(|id| ctx.memory(|memory| memory.area_rect(id)))
        .is_some_and(|rect| rect.expand(6.0).contains(p));
    (!covered).then_some(p)
}

/// Whether the plain `F` chord currently addresses a plot's data viewport:
/// the pointer rests on the data area of a plot on the active canvas. Outside
/// that context the chord keeps its board meaning (Show Selection).
pub(crate) fn pointer_in_plot_data(app: &PlotiumApp, ctx: &egui::Context) -> bool {
    let Some(ci) = app.session.active_canvas else {
        return false;
    };
    let Some(rect) = navigation_rect(ctx) else {
        return false;
    };
    let Some(p) = uncovered_pointer(app, ctx).filter(|p| rect.contains(*p)) else {
        return false;
    };
    plot_under_cursor(app, ci, rect, p)
        .is_some_and(|(_, outer, plot)| hit_zone(p, outer, plot) == HitZone::Plot)
}

/// The plot a keyboard fit addresses: the plot under the pointer when there is
/// one, otherwise the active plot object — so the palette (no meaningful
/// pointer) still acts on the plot the user is working with.
fn fit_target(app: &PlotiumApp, ctx: &egui::Context) -> Option<(usize, ObjectId)> {
    let ci = app.session.active_canvas?;
    let pointed = navigation_rect(ctx)
        .zip(uncovered_pointer(app, ctx))
        .filter(|(rect, p)| rect.contains(*p))
        .and_then(|(rect, p)| plot_under_cursor(app, ci, rect, p))
        .map(|(id, _, _)| id);
    pointed
        .or_else(|| app.doc.canvases.get(ci)?.active_plot_object_id())
        .map(|id| (ci, id))
}

/// Reset the target plot's data viewport on the requested axes as one
/// undoable step. Returns whether a plot was fitted.
pub(crate) fn fit_plot_viewport(
    app: &mut PlotiumApp,
    ctx: &egui::Context,
    axes: PlotFitAxes,
) -> bool {
    let now = ctx.input(|input| input.time);
    app.finish_pending_wheel_zoom(now, true);
    app.finish_pending_wheel_property(now, true);
    let Some((ci, object_id)) = fit_target(app, ctx) else {
        return false;
    };
    let Some(plot_object) = app.doc.canvases[ci]
        .object(object_id)
        .and_then(|object| object.plot())
    else {
        return false;
    };
    let before = plot_object.viewport.clone();
    let mut after = before.clone();
    match axes {
        PlotFitAxes::Y => after.reset_y(plot_object.figure()),
        PlotFitAxes::Both => after.reset_all(),
    }
    app.commit_object_viewport(ci, object_id, before, after);
    true
}

pub(crate) fn can_reset_peak_height(app: &PlotiumApp) -> bool {
    let Some(ci) = app.session.active_canvas else {
        return false;
    };
    let Some(id) = app
        .doc
        .canvases
        .get(ci)
        .and_then(|canvas| canvas.active_plot_object_id())
    else {
        return false;
    };
    app.doc.canvases[ci]
        .object(id)
        .is_some_and(|object| !object.locked)
        && supports_peak_height(app, ci, id)
}

pub(crate) fn reset_peak_height(app: &mut PlotiumApp, ctx: &egui::Context) {
    let Some(ci) = app.session.active_canvas else {
        return;
    };
    let Some(id) = app.doc.canvases[ci].active_plot_object_id() else {
        return;
    };
    let now = ctx.input(|input| input.time);
    app.finish_pending_wheel_zoom(now, true);
    app.finish_pending_wheel_property(now, true);
    let before = app.doc.canvases[ci]
        .object(id)
        .and_then(|object| object.plot())
        .expect("the command requires an active plot")
        .viewport
        .clone();
    let mut after = before.clone();
    after.display_gain = 1.0;
    app.commit_object_viewport(ci, id, before, after);
    app.session.status = "Reset peak height to ×1; axis ranges unchanged.".into();
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_core::state::{CanvasViewport, PlotObject};
    use plotium_figure::{Axis, Figure, Series};

    const PLOT_ID: ObjectId = ObjectId::new(1);

    /// One line plot whose trace is small inside x = 2..8 and spikes outside,
    /// so a window-scoped intensity fit is distinguishable from a full fit.
    fn line_plot_app() -> PlotiumApp {
        let mut app = PlotiumApp::new();
        let mut canvas = CanvasDocument::new("page".to_owned(), [200.0, 120.0]);
        let mut figure = Figure::new(
            "plot",
            Axis::new("x", 0.0, 10.0),
            Axis::new("y", -1.0, 100.0),
        );
        figure.series.push(Series::line(
            "trace",
            vec![
                [0.0, 100.0],
                [1.0, 90.0],
                [3.0, 1.0],
                [5.0, 2.0],
                [7.0, 3.0],
                [9.0, 80.0],
                [10.0, 100.0],
            ],
        ));
        let viewport = CanvasViewport {
            display_gain: 1.0,
            full_x: AxisRange::new(0.0, 10.0),
            full_y: AxisRange::new(-1.0, 100.0),
            view_x: AxisRange::new(2.0, 8.0),
            view_y: AxisRange::new(-50.0, 50.0),
            auto_y: false,
            full_y2: None,
            view_y2: None,
            auto_y2: true,
        };
        viewport.apply_to(&mut figure);
        canvas.objects.push(CanvasObject {
            id: PLOT_ID,
            name: "Plot".to_owned(),
            frame: ObjectFrame::new(10.0, 10.0, 180.0, 100.0),
            locked: false,
            visible: true,
            kind: CanvasObjectKind::Plot(Box::new(PlotObject::new(
                None,
                plotium_core::state::SeriesId::new(1),
                plotium_core::state::DataBinding {
                    coordinates: None,
                    series: Vec::new(),
                },
                plotium_core::state::ChartSpec::default(),
                plotium_core::state::StackSpec::default(),
                plotium_core::state::AxisProjections::default(),
                plotium_core::state::AxisOverrides::default(),
                figure,
                viewport,
            ))),
        });
        app.doc.canvases.push(canvas);
        app.session.active_canvas = Some(0);
        app
    }

    fn viewport(app: &PlotiumApp) -> CanvasViewport {
        app.doc.canvases[0]
            .object(PLOT_ID)
            .and_then(|object| object.plot())
            .expect("fixture plot")
            .viewport
            .clone()
    }

    #[test]
    fn y_fit_scales_to_the_data_visible_in_the_current_x_window() {
        let mut app = line_plot_app();
        let ctx = egui::Context::default();

        assert!(fit_plot_viewport(&mut app, &ctx, PlotFitAxes::Y));

        let fitted = viewport(&app);
        assert_eq!(fitted.view_x, AxisRange::new(2.0, 8.0));
        assert!(fitted.auto_y);
        // Only the points at x = 3, 5, 7 (y = 1..3) are inside the window; the
        // fitted y range is that span plus the auto-fit padding, far below the
        // out-of-window spikes.
        assert!((fitted.view_y.min - 0.9).abs() < 1e-9);
        assert!((fitted.view_y.max - 3.16).abs() < 1e-9);

        app.undo();
        let restored = viewport(&app);
        assert_eq!(restored.view_y, AxisRange::new(-50.0, 50.0));
        assert!(!restored.auto_y);
    }

    #[test]
    fn both_axes_fit_resets_the_full_data_range_undoably() {
        let mut app = line_plot_app();
        let ctx = egui::Context::default();

        assert!(fit_plot_viewport(&mut app, &ctx, PlotFitAxes::Both));

        let fitted = viewport(&app);
        assert_eq!(fitted.view_x, AxisRange::new(0.0, 10.0));
        assert_eq!(fitted.view_y, AxisRange::new(-1.0, 100.0));
        assert!(fitted.auto_y);

        app.undo();
        assert_eq!(viewport(&app).view_x, AxisRange::new(2.0, 8.0));
    }

    #[test]
    fn fit_without_a_plot_reports_no_target() {
        let mut app = PlotiumApp::new();
        app.doc
            .canvases
            .push(CanvasDocument::new("empty".to_owned(), [100.0, 80.0]));
        app.session.active_canvas = Some(0);
        let ctx = egui::Context::default();

        assert!(!fit_plot_viewport(&mut app, &ctx, PlotFitAxes::Both));
    }
}
