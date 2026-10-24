use super::*;

pub(crate) fn peak_height_label(gain: f64) -> String {
    if (0.001..1000.0).contains(&gain) {
        format!("×{gain:.3}")
    } else {
        format!("×{gain:.3e}")
    }
}

pub(crate) fn supports_peak_height(app: &PlotiumApp, canvas: usize, object: ObjectId) -> bool {
    app.doc.canvases[canvas]
        .object(object)
        .and_then(|object| object.plot())
        .is_some_and(|plot| {
            !plot
                .binding
                .coordinates
                .as_ref()
                .is_some_and(|coordinates| coordinates.transposed)
                && plot.figure().supports_display_gain()
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn scale_line_intensity(
    app: &mut PlotiumApp,
    canvas: usize,
    object: ObjectId,
    plot: PlotRect,
    pointer: Pos2,
    scroll_y: f32,
    now: f64,
    ui: &Ui,
) -> bool {
    let Some(plot_object) = app.doc.canvases[canvas]
        .object(object)
        .and_then(|object| object.plot())
    else {
        return false;
    };
    if plot_object.figure().y.categories.is_some() {
        app.session.status =
            "This categorical plot has no continuous display-intensity scale.".to_owned();
        return true;
    }
    if supports_peak_height(app, canvas, object) {
        let gain = (plot_object.viewport.display_gain
            * f64::from((scroll_y * WHEEL_ZOOM_SPEED).exp()))
        .clamp(
            plotium_figure::MIN_DISPLAY_GAIN,
            plotium_figure::MAX_DISPLAY_GAIN,
        );
        if let Err(error) = plot_object.figure().check_display_gain(gain) {
            app.session.status = format!("Could not adjust peak height: {error}");
            return true;
        }
        if !update_pending_viewport_edit(app, canvas, object, now) {
            return false;
        }
        let plot_object = app.doc.canvases[canvas]
            .object_mut(object)
            .and_then(|object| object.plot_mut())
            .expect("the wheel target was just validated");
        // Freeze the drawn window, including any space reserved for a legend.
        // Amplification must not trigger an automatic refit or move a baseline.
        plot_object.viewport.view_x =
            AxisRange::new(plot_object.figure().x.min, plot_object.figure().x.max);
        plot_object.viewport.view_y =
            AxisRange::new(plot_object.figure().y.min, plot_object.figure().y.max);
        plot_object.viewport.auto_y = false;
        plot_object.viewport.display_gain = gain;
        plot_object.apply_viewport();
        app.mark_document_dirty();
        app.session.status = format!(
            "Peak height {} · baselines fixed · View > Reset Peak Height",
            peak_height_label(gain)
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
        return true;
    }
    let before = plot_object.viewport.clone();
    let y_axis = plot_object.figure().y.clone();
    let view = before.view_y;
    let anchor = if view.min <= 0.0 && view.max >= 0.0 {
        0.0
    } else if y_axis.scale == plotium_figure::AxisScale::Log10 {
        (view.min * view.max).sqrt()
    } else {
        (view.min + view.max) * 0.5
    };
    let scale = f64::from((-scroll_y * WHEEL_ZOOM_SPEED).exp()).clamp(0.2, 5.0);
    zoom_plot_viewport(
        app,
        canvas,
        object,
        plot_rect(plot),
        plot,
        Pos2::new(pointer.x, to_screen_y(anchor, plot, &y_axis)),
        scale,
        (false, true),
        now,
        ui,
    );
    app.session.status =
        "Adjusted plot intensity; automatic Y scaling is now off. Double-click Y to reset."
            .to_owned();
    true
}
