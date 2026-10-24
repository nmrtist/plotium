use super::*;
use plotium_core::state::{
    ManualPeakSnap, PeakBandDrag, PeakSet, PeakThresholdDrag, ResolvedPeak, Trace1d,
};

const PEAK_GRAB_PX: f32 = 10.0;
const LINE_GRAB_PX: f32 = 6.0;
const DRAG_DEADZONE_PX: f32 = 4.0;
/// Pixel radius of the manual-pick apex search. Fixed in screen space so
/// zooming in narrows the data window along with the click precision it
/// affords, letting weak lines be picked next to strong ones.
const PEAK_SNAP_PX: f32 = 12.0;

/// The snap for a manual pick: an apex search over ± `PEAK_SNAP_PX` at the
/// current zoom, or the nearest sample while `Shift` is held (free placement
/// for shoulders the apex search refuses to land on).
/// The data width is measured around the pick position `px`, which on a log
/// axis differs from one end of the plot to the other.
pub(crate) fn manual_peak_snap(
    shift: bool,
    plot: PlotRect,
    x_axis: &plotium_figure::Axis,
    px: f32,
) -> ManualPeakSnap {
    if shift {
        return ManualPeakSnap::NearestSample;
    }
    let half_width = if x_axis.effective_scale().is_linear() {
        x_axis.span().abs() / f64::from(plot.width.max(1.0)) * f64::from(PEAK_SNAP_PX)
    } else {
        let low = from_screen_x(px - PEAK_SNAP_PX, plot, x_axis);
        let high = from_screen_x(px + PEAK_SNAP_PX, plot, x_axis);
        (high - low).abs() * 0.5
    };
    ManualPeakSnap::Apex { half_width }
}

fn peak_hit(resolved: &[ResolvedPeak], sc: &Screen, p: Pos2) -> Option<u64> {
    let mut best: Option<(u64, f32)> = None;
    for peak in resolved {
        let Some(id) = peak.mark_id else { continue };
        let d = Pos2::new(sc.x(peak.x), sc.y(peak.y)).distance(p);
        if d <= PEAK_GRAB_PX && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((id, d));
        }
    }
    best.map(|(id, _)| id)
}

fn threshold_y(peaks: &PeakSet, trace: &Trace1d) -> f64 {
    peaks
        .detector
        .threshold
        .unwrap_or_else(|| PeakSet::auto_threshold(trace))
}

struct Screen {
    gain: f64,
    plot: PlotRect,
    x_axis: plotium_figure::Axis,
    y_axis: plotium_figure::Axis,
}

impl Screen {
    fn x(&self, x: f64) -> f32 {
        to_screen_x(x, self.plot, &self.x_axis)
    }
    fn y(&self, y: f64) -> f32 {
        to_screen_y(y * self.gain, self.plot, &self.y_axis)
    }
    fn to_x(&self, px: f32) -> f64 {
        from_screen_x(px, self.plot, &self.x_axis)
    }
    fn to_y(&self, py: f32) -> f64 {
        from_screen_y(py, self.plot, &self.y_axis) / self.gain
    }
}

pub(crate) fn handle_peaks(
    app: &mut PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    dataset: usize,
    plot: PlotRect,
    ui: &Ui,
    resp: &egui::Response,
) {
    let column = app.session.ui.peak_column;
    let Some(trace) = app
        .doc
        .datasets
        .get(dataset)
        .and_then(|d| d.displayed_trace(column))
    else {
        return;
    };
    let Some(peaks) = app
        .doc
        .datasets
        .get(dataset)
        .and_then(Dataset::peaks)
        .cloned()
    else {
        return;
    };

    let fig = app.doc.canvases[ci]
        .object(object_id)
        .and_then(|object| object.plot())
        .unwrap()
        .figure()
        .clone();
    let sc = Screen {
        gain: fig.display_gain,
        plot,
        x_axis: fig.x,
        y_axis: fig.y,
    };

    let (hover, pressed, down, released, del, esc, shift) = ui.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            i.key_pressed(egui::Key::Escape),
            i.modifiers.shift,
        )
    });

    let resolved = peaks.resolve(app.doc.datasets[dataset].peak_reference_offset_ppm());
    peak_context_menu(app, dataset, &resolved, &sc, hover, resp);

    if esc {
        if matches!(
            app.interaction(),
            Interaction::PeakThreshold(_) | Interaction::PeakBand(_)
        ) {
            app.cancel_interaction();
        } else {
            app.session.ui.selected_peak = None;
        }
        return;
    }
    if del && let Some(id) = app.session.ui.selected_peak {
        app.remove_peak(dataset, id);
        return;
    }

    if let Some((dc, dobj)) = peak_drag_target(app.interaction())
        && (dc != ci || dobj != object_id)
    {
        return;
    }
    if matches!(app.interaction(), Interaction::PeakThreshold(_)) {
        if let Some(p) = hover
            && let Interaction::PeakThreshold(drag) = &mut app.session.ui.interaction
        {
            drag.y = sc.to_y(p.y.clamp(plot.top, plot.bottom()));
        }
        if released || !down {
            finish_threshold_drag(app, dataset, column);
        }
        return;
    }
    if matches!(app.interaction(), Interaction::PeakBand(_)) {
        if let Some(p) = hover
            && let Interaction::PeakBand(drag) = &mut app.session.ui.interaction
        {
            drag.current_x = sc.to_x(p.x.clamp(plot.left, plot.right()));
        }
        if released || !down {
            finish_band_drag(app, dataset, &sc, column, shift);
        }
        return;
    }

    let Some(p) = hover.filter(|p| plot_contains(plot, *p)) else {
        return;
    };

    let hit = peak_hit(&resolved, &sc, p);
    let on_line = (p.y - sc.y(threshold_y(&peaks, &trace))).abs() <= LINE_GRAB_PX;
    ui.ctx().set_cursor_icon(if hit.is_some() {
        egui::CursorIcon::Default
    } else if on_line {
        egui::CursorIcon::ResizeVertical
    } else {
        egui::CursorIcon::Crosshair
    });

    if pressed {
        match hit {
            Some(id) => app.session.ui.selected_peak = Some(id),
            None if on_line => {
                app.session.ui.selected_peak = None;
                app.begin_interaction(Interaction::PeakThreshold(PeakThresholdDrag {
                    canvas: ci,
                    object: object_id,
                    dataset,
                    y: threshold_y(&peaks, &trace),
                }));
            }
            None => {
                let x = sc.to_x(p.x);
                app.session.ui.selected_peak = None;
                app.begin_interaction(Interaction::PeakBand(PeakBandDrag {
                    canvas: ci,
                    object: object_id,
                    dataset,
                    anchor_x: x,
                    current_x: x,
                }));
            }
        }
    }
}

fn peak_drag_target(interaction: &Interaction) -> Option<(usize, ObjectId)> {
    match interaction {
        Interaction::PeakThreshold(d) => Some((d.canvas, d.object)),
        Interaction::PeakBand(d) => Some((d.canvas, d.object)),
        _ => None,
    }
}

fn finish_threshold_drag(
    app: &mut PlotiumApp,
    dataset: usize,
    column: Option<plotium_core::data::ColumnId>,
) {
    let Interaction::PeakThreshold(drag) = app.take_interaction() else {
        return;
    };
    app.run_detection(dataset, Some(drag.y), column);
}

/// A band wider than the click dead-zone picks every peak inside it; a narrower one
/// is a plain click that places a single snapped peak (`Shift` skips the snap).
fn finish_band_drag(
    app: &mut PlotiumApp,
    dataset: usize,
    sc: &Screen,
    column: Option<plotium_core::data::ColumnId>,
    shift: bool,
) {
    let Interaction::PeakBand(drag) = app.take_interaction() else {
        return;
    };
    if (sc.x(drag.anchor_x) - sc.x(drag.current_x)).abs() < DRAG_DEADZONE_PX {
        let snap = manual_peak_snap(shift, sc.plot, &sc.x_axis, sc.x(drag.anchor_x));
        app.add_manual_peak(dataset, drag.anchor_x, column, snap);
    } else {
        app.add_peaks_in_range(dataset, drag.anchor_x, drag.current_x, column);
    }
}

fn peak_context_menu(
    app: &mut PlotiumApp,
    dataset: usize,
    resolved: &[ResolvedPeak],
    sc: &Screen,
    hover: Option<Pos2>,
    resp: &egui::Response,
) {
    if resp.secondary_clicked()
        && let Some(p) = hover
    {
        app.session.ui.selected_peak = peak_hit(resolved, sc, p);
    }
    resp.context_menu(|ui| {
        let Some(p) = hover else {
            ui.close();
            return;
        };
        let Some(id) = peak_hit(resolved, sc, p) else {
            ui.close();
            return;
        };
        if ui.button("Delete peak").clicked() {
            app.remove_peak(dataset, id);
            ui.close();
        }
    });
}
