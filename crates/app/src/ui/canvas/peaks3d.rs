use super::*;

pub(crate) fn handle_volume_peaks(
    app: &mut PlotiumApp,
    ci: usize,
    object: ObjectId,
    di: usize,
    plot: PlotRect,
    ui: &Ui,
    response: &egui::Response,
) {
    let Some(nmr) = app.doc.datasets[di].as_nmr2d() else {
        return;
    };
    let Some(volume) = nmr.volume.as_ref() else {
        return;
    };
    let dataset = app.doc.datasets[di].resource_id();
    let Some(figure) = app.doc.canvases[ci]
        .object(object)
        .and_then(|o| o.plot())
        .map(|p| p.figure())
    else {
        return;
    };
    let x = figure.x.clone();
    let y = figure.y.clone();
    let axes: Vec<_> = (0..3).filter(|a| *a != volume.fixed_axis).collect();
    let marks: Vec<_> = volume
        .peaks
        .marks
        .iter()
        .filter(|m| {
            volume
                .peak_indices(m.ppm)
                .is_ok_and(|i| i[volume.fixed_axis] == volume.index)
        })
        .cloned()
        .collect();
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Delete))
        && let Some((owner, id)) = app.session.ui.selected_peak_3d
        && owner == dataset
    {
        if let Err(e) = app.delete_peak_3d(dataset, id) {
            app.session.status = e;
        }
        return;
    }
    let Some(pointer) = response
        .interact_pointer_pos()
        .filter(|p| plot_contains(plot, *p))
    else {
        return;
    };
    let hit = marks.iter().find(|m| {
        Pos2::new(
            to_screen_x(m.ppm[axes[1]], plot, &x),
            to_screen_y(m.ppm[axes[0]], plot, &y),
        )
        .distance(pointer)
            < 9.0
    });
    let drag_id = egui::Id::new(("peak3d_drag", dataset, object));
    if response.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        ui.ctx().data_mut(|d| {
            d.remove::<plotium_core::state::Peak3DId>(drag_id);
            if let Some(origin) = origin
                && let Some(mark) = marks.iter().find(|m| {
                    Pos2::new(
                        to_screen_x(m.ppm[axes[1]], plot, &x),
                        to_screen_y(m.ppm[axes[0]], plot, &y),
                    )
                    .distance(origin)
                        < 9.
                })
            {
                app.session.ui.selected_peak_3d = Some((dataset, mark.id));
                d.insert_temp(drag_id, mark.id);
            }
        });
    }
    if response.clicked() {
        if let Some(mark) = hit {
            app.session.ui.selected_peak_3d = Some((dataset, mark.id));
        } else {
            let mut ppm = [0.; 3];
            ppm[volume.fixed_axis] = match volume
                .processed
                .axis_coordinate(volume.fixed_axis, volume.index)
            {
                Ok(v) => v,
                Err(e) => {
                    app.session.status = e;
                    return;
                }
            };
            ppm[axes[1]] = from_screen_x(pointer.x, plot, &x);
            ppm[axes[0]] = from_screen_y(pointer.y, plot, &y);
            match app.add_peak_3d(dataset, ppm) {
                Ok(id) => app.session.ui.selected_peak_3d = Some((dataset, id)),
                Err(e) => app.session.status = e,
            }
        }
    }
    if response.drag_stopped()
        && let Some(id) = ui.ctx().data_mut(|d| {
            let p = d.get_temp::<plotium_core::state::Peak3DId>(drag_id);
            d.remove::<plotium_core::state::Peak3DId>(drag_id);
            p
        })
        && let Some(mark) = marks.iter().find(|m| m.id == id)
    {
        let mut ppm = mark.ppm;
        ppm[axes[1]] = from_screen_x(pointer.x, plot, &x);
        ppm[axes[0]] = from_screen_y(pointer.y, plot, &y);
        if let Err(e) = app.edit_peak_3d(dataset, id, ppm, mark.note.clone()) {
            app.session.status = e;
        }
    }
}

pub(crate) fn paint_volume_peaks(
    app: &PlotiumApp,
    ci: usize,
    object: ObjectId,
    di: usize,
    plot: PlotRect,
    painter: &egui::Painter,
) {
    let Some(nmr) = app.doc.datasets[di].as_nmr2d() else {
        return;
    };
    let Some(volume) = nmr.volume.as_ref() else {
        return;
    };
    let dataset = app.doc.datasets[di].resource_id();
    let Some(figure) = app.doc.canvases[ci]
        .object(object)
        .and_then(|o| o.plot())
        .map(|p| p.figure())
    else {
        return;
    };
    let x = figure.x.clone();
    let y = figure.y.clone();
    let axes: Vec<_> = (0..3).filter(|a| *a != volume.fixed_axis).collect();
    let marks: Vec<_> = volume
        .peaks
        .marks
        .iter()
        .filter(|m| {
            volume
                .peak_indices(m.ppm)
                .is_ok_and(|i| i[volume.fixed_axis] == volume.index)
        })
        .cloned()
        .collect();
    for mark in &marks {
        let pos = Pos2::new(
            to_screen_x(mark.ppm[axes[1]], plot, &x),
            to_screen_y(mark.ppm[axes[0]], plot, &y),
        );
        if plot_contains(plot, pos) {
            let selected = app.session.ui.selected_peak_3d == Some((dataset, mark.id));
            let color = if selected {
                Color32::YELLOW
            } else {
                Color32::LIGHT_GREEN
            };
            painter.line_segment(
                [pos - Vec2::new(5., 0.), pos + Vec2::new(5., 0.)],
                Stroke::new(1.5_f32, color),
            );
            painter.line_segment(
                [pos - Vec2::new(0., 5.), pos + Vec2::new(0., 5.)],
                Stroke::new(1.5_f32, color),
            );
        }
    }
}

#[cfg(test)]
#[path = "peaks3d_tests.rs"]
mod tests;
