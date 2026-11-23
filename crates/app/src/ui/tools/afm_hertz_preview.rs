use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui};
use plotium_analysis::afm_hertz::{HertzInput, HertzResult, select_hertz_points};
use plotium_core::state::{AfmHertzDraft, AfmHertzReference, AfmHertzWindow};

struct Colors {
    baseline: Color32,
    fit: Color32,
    contact: Color32,
}

impl Colors {
    fn from_ui(ui: &Ui) -> Self {
        if ui.visuals().dark_mode {
            Self {
                baseline: Color32::from_rgb(76, 199, 172),
                fit: Color32::from_rgb(122, 188, 252),
                contact: Color32::from_rgb(252, 164, 82),
            }
        } else {
            Self {
                baseline: Color32::from_rgb(6, 113, 91),
                fit: Color32::from_rgb(24, 101, 177),
                contact: Color32::from_rgb(157, 73, 10),
            }
        }
    }
}

pub(super) fn approach(
    ui: &mut Ui,
    points: &[[f64; 2]],
    draft: &AfmHertzDraft,
    spring: f64,
    suggestion: Option<HertzInput>,
) -> Option<f64> {
    let colors = Colors::from_ui(ui);
    let relative_z = draft.window == AfmHertzWindow::RelativeZ;
    let contact = (!draft.linearized)
        .then(|| finite_number(&draft.contact_z_nm))
        .flatten();
    let baseline = band(&draft.baseline_z_nm, colors.baseline);
    let fit = relative_z
        .then(|| band(&draft.fit_z_nm, colors.fit))
        .flatten();
    let selection = match draft.options(spring) {
        Ok(options) => match select_hertz_points(points, options) {
            Ok(selection) => Some(selection),
            Err(error) => {
                ui.small(error.to_string());
                None
            }
        },
        Err(error) => {
            if !relative_z || draft.reference != AfmHertzReference::Zero {
                ui.small(error);
            }
            None
        }
    };
    let corrected = !relative_z && selection.is_some();
    let display_points = if corrected {
        selection
            .as_ref()
            .map_or(points, |selection| selection.corrected.as_slice())
    } else {
        points
    };
    let selected: Vec<_> = selection
        .iter()
        .flat_map(|selection| selection.indices.iter().map(|&index| display_points[index]))
        .collect();
    let force_bounds = selection
        .as_ref()
        .and_then(|selection| selection.force_bounds_nn);
    if let Some(selection) = &selection {
        ui.small(format!(
            "Reference force: {:.6} nN · {} selected points",
            selection.reference_nn,
            selected.len()
        ));
    }
    if let Some([low, high]) = force_bounds {
        ui.small(format!(
            "Force window: {} to {} nN",
            value(low),
            value(high)
        ));
    }
    ui.horizontal_wrapped(|ui| {
        key(ui, "Baseline", colors.baseline);
        key(ui, "Fit window", colors.fit);
        if !draft.linearized {
            key(ui, "Contact Z", colors.contact);
        }
        if suggestion.is_some() {
            ui.weak("Dashed: suggested inputs");
        }
    });
    let trace = Trace {
        color: ui.visuals().text_color(),
        points: display_points,
    };
    let (response, scale) = plot(
        ui,
        [
            "Relative Z (nm)",
            if corrected {
                "Corrected force (nN)"
            } else {
                "Force (nN)"
            },
        ],
        &[trace],
        Marks {
            bands: [baseline, fit],
            vertical: contact.map(|z| (z, colors.contact)),
            suggestion,
            horizontal: force_bounds.map(|bounds| (bounds, colors.fit)),
            selected: Some((&selected, colors.fit)),
            pick: !draft.linearized,
            ..Default::default()
        },
    )?;
    if draft.linearized {
        return None;
    }
    let response = response
        .on_hover_cursor(egui::CursorIcon::Crosshair)
        .on_hover_text("Set contact Z at the nearest sample");
    if !response.clicked() {
        return None;
    }
    let pointer = response.interact_pointer_pos()?;
    if !scale.rect.contains(pointer) {
        return None;
    }
    let z = scale.data_x(pointer.x);
    points
        .iter()
        .filter(|p| finite(p))
        .map(|p| p[0])
        .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()))
}

pub(super) fn result(ui: &mut Ui, result: &HertzResult) {
    let colors = Colors::from_ui(ui);
    let observed = result
        .rows
        .iter()
        .map(|row| [row.indentation_nm, row.corrected_force_nn])
        .collect::<Vec<_>>();
    let predicted = result
        .rows
        .iter()
        .map(|row| [row.indentation_nm, row.predicted_force_nn])
        .collect::<Vec<_>>();
    let residual = result
        .rows
        .iter()
        .map(|row| [row.indentation_nm, row.residual_nn()])
        .collect::<Vec<_>>();
    let observed_color = ui.visuals().text_color();
    ui.horizontal_wrapped(|ui| {
        key(ui, "Measured", observed_color);
        key(ui, "Hertz", colors.fit);
    });
    let _ = plot(
        ui,
        ["Indentation (nm)", "Corrected force (nN)"],
        &[
            Trace {
                color: observed_color,
                points: &observed,
            },
            Trace {
                color: colors.fit,
                points: &predicted,
            },
        ],
        Marks::default(),
    );
    let _ = plot(
        ui,
        ["Indentation (nm)", "Residual (nN)"],
        &[Trace {
            color: colors.contact,
            points: &residual,
        }],
        Marks {
            zero_line: true,
            ..Default::default()
        },
    );
}

fn key(ui: &mut Ui, label: &str, color: Color32) {
    ui.colored_label(color, label);
}

#[derive(Clone, Copy)]
struct Trace<'a> {
    color: Color32,
    points: &'a [[f64; 2]],
}

#[derive(Default)]
struct Marks<'a> {
    bands: [Option<([f64; 2], Color32)>; 2],
    vertical: Option<(f64, Color32)>,
    suggestion: Option<HertzInput>,
    horizontal: Option<([f64; 2], Color32)>,
    selected: Option<(&'a [[f64; 2]], Color32)>,
    zero_line: bool,
    pick: bool,
}

#[derive(Clone, Copy)]
struct Scale {
    x: [f64; 2],
    y: [f64; 2],
    rect: Rect,
}

impl Scale {
    fn point(self, [x, y]: [f64; 2]) -> Pos2 {
        egui::pos2(
            self.screen_x(x),
            self.rect.bottom()
                - ((y - self.y[0]) / (self.y[1] - self.y[0])) as f32 * self.rect.height(),
        )
    }

    fn screen_x(self, x: f64) -> f32 {
        self.rect.left() + ((x - self.x[0]) / (self.x[1] - self.x[0])) as f32 * self.rect.width()
    }

    fn data_x(self, x: f32) -> f64 {
        self.x[0] + f64::from((x - self.rect.left()) / self.rect.width()) * (self.x[1] - self.x[0])
    }
}

fn plot(
    ui: &mut Ui,
    [x_name, y_name]: [&str; 2],
    traces: &[Trace<'_>],
    marks: Marks<'_>,
) -> Option<(Response, Scale)> {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for point in traces
        .iter()
        .flat_map(|trace| trace.points)
        .filter(|point| finite(point))
    {
        for j in 0..2 {
            lo[j] = lo[j].min(point[j]);
            hi[j] = hi[j].max(point[j]);
        }
    }
    let (mut y_min, mut y_max) = (lo[1], hi[1]);
    if marks.zero_line {
        y_min = y_min.min(0.0);
        y_max = y_max.max(0.0);
    }
    if !lo[0].is_finite() || lo[0] >= hi[0] || !(hi[0] - lo[0]).is_finite() {
        ui.weak("Preview unavailable: no finite X span.");
        return None;
    }
    if let Some(([low, high], _)) = marks.horizontal {
        y_min = y_min.min(low);
        y_max = y_max.max(high);
    }
    if y_min == y_max {
        let pad = (y_min.abs() * 0.01).max(0.01);
        y_min -= pad;
        y_max += pad;
    }
    if !(y_max - y_min).is_finite() {
        ui.weak("Preview unavailable: no finite Y span.");
        return None;
    }
    ui.small(y_name);
    let size = egui::vec2(ui.available_width().max(70.0), 112.0);
    let response = ui.allocate_response(
        size,
        if marks.pick {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let rect = response.rect;
    let plot = Rect::from_min_max(
        rect.min + egui::vec2(53.0, 8.0),
        rect.max - egui::vec2(6.0, 17.0),
    );
    let scale = Scale {
        x: [lo[0], hi[0]],
        y: [y_min, y_max],
        rect: plot,
    };
    let painter = ui.painter_at(rect);
    let ink = ui.visuals().weak_text_color();
    let axis = Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color);
    painter.rect_filled(plot, 0.0, ui.visuals().faint_bg_color);
    let chart = painter.with_clip_rect(plot);
    for ([min, max], color) in marks.bands.into_iter().flatten() {
        let min = scale.screen_x(min.clamp(lo[0], hi[0]));
        let max = scale.screen_x(max.clamp(lo[0], hi[0]));
        chart.rect_filled(
            Rect::from_min_max(egui::pos2(min, plot.top()), egui::pos2(max, plot.bottom())),
            0.0,
            color.gamma_multiply(0.21),
        );
    }
    if marks.zero_line {
        chart.line_segment([scale.point([lo[0], 0.0]), scale.point([hi[0], 0.0])], axis);
    }
    if let Some((z, color)) = marks.vertical
        && scale.x[0] <= z
        && z <= scale.x[1]
    {
        let x = scale.screen_x(z);
        chart.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(1.5_f32, color),
        );
    }
    for trace in traces {
        // Break the line across invalid points instead of hiding gaps.
        for segment in trace.points.split(|point| !finite(point)) {
            if segment.len() >= 2 {
                chart.add(egui::Shape::line(
                    segment.iter().map(|&point| scale.point(point)).collect(),
                    Stroke::new(1.3_f32, trace.color),
                ));
            }
        }
    }
    if let Some((bounds, color)) = marks.horizontal {
        for force in bounds {
            chart.line_segment(
                [scale.point([lo[0], force]), scale.point([hi[0], force])],
                Stroke::new(1.0_f32, color),
            );
        }
    }
    if let Some((points, color)) = marks.selected {
        for &point in points {
            chart.circle_filled(scale.point(point), 2.1, color);
        }
    }
    if let Some(suggestion) = marks.suggestion {
        draw_suggestion(&chart, scale, suggestion, &Colors::from_ui(ui));
    }
    painter.line_segment([plot.left_top(), plot.left_bottom()], axis);
    painter.line_segment([plot.left_bottom(), plot.right_bottom()], axis);
    for (anchor, alignment, tick) in [
        (
            plot.left_top() - egui::vec2(4.0, 0.0),
            Align2::RIGHT_CENTER,
            y_max,
        ),
        (
            plot.left_bottom() - egui::vec2(4.0, 0.0),
            Align2::RIGHT_CENTER,
            y_min,
        ),
        (
            plot.left_bottom() + egui::vec2(0.0, 3.0),
            Align2::LEFT_TOP,
            lo[0],
        ),
        (
            plot.right_bottom() + egui::vec2(0.0, 3.0),
            Align2::RIGHT_TOP,
            hi[0],
        ),
    ] {
        painter.text(anchor, alignment, value(tick), FontId::monospace(9.0), ink);
    }
    ui.small(x_name);
    Some((response, scale))
}

fn draw_suggestion(chart: &egui::Painter, scale: Scale, input: HertzInput, colors: &Colors) {
    let dashed = |points: &[Pos2], color| {
        chart.extend(egui::Shape::dashed_line(
            points,
            Stroke::new(1.75_f32, color),
            4.0,
            3.0,
        ));
    };
    let plot = scale.rect;
    for ([lo, hi], color) in [
        (input.baseline_z_nm, colors.baseline),
        (input.fit_z_nm, colors.fit),
    ] {
        let left = scale.screen_x(lo.clamp(scale.x[0], scale.x[1]));
        let right = scale.screen_x(hi.clamp(scale.x[0], scale.x[1]));
        if left < right {
            // Inset the outline so the dash pattern is visible at plot limits.
            let top = plot.top() + 2.0;
            let bottom = plot.bottom() - 2.0;
            dashed(
                &[
                    egui::pos2(left, top),
                    egui::pos2(right, top),
                    egui::pos2(right, bottom),
                    egui::pos2(left, bottom),
                    egui::pos2(left, top),
                ],
                color,
            );
        }
    }
    let x = scale.screen_x(input.contact_z_nm);
    dashed(
        &[egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
        colors.contact,
    );
}

fn band(limits: &[String; 2], color: Color32) -> Option<([f64; 2], Color32)> {
    let min = finite_number(&limits[0])?;
    let max = finite_number(&limits[1])?;
    (min < max).then_some(([min, max], color))
}

fn finite_number(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn finite(point: &[f64; 2]) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

pub(super) fn value(value: f64) -> String {
    if value != 0.0 && !(0.01..1000.0).contains(&value.abs()) {
        format!("{value:.3e}")
    } else {
        format!("{value:.3}")
    }
}
