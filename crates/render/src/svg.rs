use crate::{
    Document, DocumentItem, DocumentObject, Projector, Rect, TICK_LABEL_PAD, TextAnchor,
    axis_layout_placed, error_bar_segments, heatmap_cells, integral, minor_tick_values,
    polygon_outline, projection_points, range_label_layout,
};
use plotium_figure::{AxisFrame, AxisTrace, Figure};
use std::fmt::Write as _;

pub(crate) mod color_scale;
pub(crate) mod document;
#[cfg(all(windows, feature = "emf"))]
pub(crate) use document::prepared_raster;
mod arrows;
mod curves;
mod legend;
mod overlay;
mod preview;
mod sticks;
pub use document::{export_document, export_document_for_bounds, export_document_page};
use overlay::write_overlay;
pub use preview::{PreviewDownsample, PreviewSvg, export_document_preview};

pub fn export(fig: &Figure) -> String {
    let w = fig.width;
    let h = fig.height;
    let outer = Rect::new(0.0, 0.0, w, h);
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{font_family}">"#,
        font_family = crate::rich_text::SVG_FONT_FAMILY,
    );
    write_figure(
        &mut s,
        fig,
        outer,
        "plot",
        None,
        Default::default(),
        None,
        None,
    );
    let _ = write!(s, "</svg>");
    s
}

fn write_document_object(
    s: &mut String,
    object: &DocumentObject<'_>,
    omit_figure_background_matching: Option<plotium_figure::Color>,
    lod: Option<&mut preview::PreviewLod>,
) {
    if !object.visible {
        return;
    }
    let id = escape_id(&object.id);
    let _ = write!(
        s,
        r#"<g id="{id}" transform="translate({x:.2},{y:.2})">"#,
        x = object.frame.left,
        y = object.frame.top,
    );
    write_figure(
        s,
        object.figure,
        Rect::new(0.0, 0.0, object.frame.width, object.frame.height),
        &format!("{id}_clip"),
        omit_figure_background_matching,
        object.y_title_clearance,
        object.stack.as_ref(),
        lod,
    );
    if let Some(title) = &object.title {
        write_panel_letter(s, &title.text, title.position, title.font_size);
    }
    let _ = write!(s, "</g>");
}

#[allow(clippy::too_many_arguments)]
fn write_figure(
    s: &mut String,
    fig: &Figure,
    frame: Rect,
    clip_id: &str,
    omit_background_matching: Option<plotium_figure::Color>,
    y_title_clearance: crate::YTitleClearance,
    stack: Option<&crate::StackPlacement>,
    mut lod: Option<&mut preview::PreviewLod>,
) {
    let ty = fig.typography;
    // Plot text inherits the figure face; explicitly bold text stays bold.
    let _ = write!(
        s,
        r#"<g font-family="{family}"{weight}>"#,
        family = crate::rich_text::svg_font_family(ty.typeface),
        weight = if ty.font_weight.is_bold() {
            r#" font-weight="bold""#
        } else {
            ""
        },
    );
    let w = frame.width;
    let h = frame.height;
    let (outer, y_title_clearance) = crate::y_title_area(fig, frame, y_title_clearance, 1.0);
    let layout = axis_layout_placed(fig, outer.width, outer.height, stack);
    let xf = crate::XFurniture::of(fig, stack);
    let proj = Projector::new(fig, outer, &layout.margins);
    let plot = proj.plot;
    let x_lane_bottom = outer.top + h - proj.bottom_letterbox(outer, &layout.margins);
    let y_title_cy = crate::y_title_center(fig, plot, outer, y_title_clearance, 1.0);
    let y_title_left = crate::y_title_left(fig, &layout, &proj, outer, y_title_cy, stack, 1.0);

    if omit_background_matching != Some(fig.background) {
        let _ = write!(
            s,
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{}"/>"#,
            fig.background.to_hex(),
            x = frame.left,
            y = frame.top + xf.background_inset,
            h = h - xf.background_inset,
        );
    }

    if xf.show_title && !fig.title.trim().is_empty() {
        let _ = write!(
            s,
            r#"<text x="{cx}" y="{y}" text-anchor="middle" font-size="{font}" fill="{col}">{title}</text>"#,
            cx = frame.left + w / 2.0,
            y = frame.top + crate::rich_text::below_top(&fig.title, ty.title_pt),
            font = ty.title_pt,
            col = plotium_figure::Color::BLACK.to_hex(),
            title = svg_content(&fig.title, ty.title_pt),
        );
    }

    let hidden_frame = fig.axis_frame == AxisFrame::Hidden;
    let (x_ticks, y_ticks) = (&layout.x_ticks, &layout.y_ticks);

    if fig.show_grid && !hidden_frame {
        let grid = plotium_figure::Color::GRID.to_hex();
        for &xt in &x_ticks.values {
            let (px, _) = proj.project([xt, fig.y.min]);
            let _ = write!(
                s,
                r#"<line x1="{px:.2}" y1="{t:.2}" x2="{px:.2}" y2="{b:.2}" stroke="{grid}" stroke-width="1"/>"#,
                t = plot.top,
                b = plot.bottom(),
            );
        }
        for &yt in &y_ticks.values {
            let (_, py) = proj.project([fig.x.min, yt]);
            let _ = write!(
                s,
                r#"<line x1="{l:.2}" y1="{py:.2}" x2="{r:.2}" y2="{py:.2}" stroke="{grid}" stroke-width="1"/>"#,
                l = plot.left,
                r = plot.right(),
            );
        }
    }

    let axis = plotium_figure::Color::AXIS.to_hex();
    let y_axis_x = proj.left_band.map(|band| band.left).unwrap_or(plot.left);

    let minor = ty.minor_tick_reach();
    let mut minor_path = String::new();
    for xt in minor_tick_values(&fig.x, x_ticks, ty.minor_ticks) {
        let (px, _) = proj.project([xt, fig.y.min]);
        let _ = write!(
            minor_path,
            "M{px:.2} {b:.2}v{minor}",
            b = plot.bottom(),
            minor = xf.minor_tick_reach,
        );
    }
    for yt in minor_tick_values(&fig.y, y_ticks, ty.minor_ticks) {
        let (_, py) = proj.project([fig.x.min, yt]);
        let _ = write!(
            minor_path,
            "M{yl:.2} {py:.2}h{minor}",
            yl = y_axis_x - minor
        );
    }
    if !minor_path.is_empty() {
        let _ = write!(
            s,
            r#"<path d="{minor_path}" stroke="{axis}" stroke-width="{width}"/>"#,
            width = ty.axis_line_pt,
        );
    }
    for (&xt, label) in x_ticks.values.iter().zip(&x_ticks.labels) {
        let (px, _) = proj.project([xt, fig.y.min]);
        let _ = write!(
            s,
            r#"<path d="M{px:.2} {b:.2}v{tick}" stroke="{axis}" stroke-width="{width}"/>"#,
            b = plot.bottom(),
            tick = xf.tick_reach,
            width = ty.axis_line_pt,
        );
        if xf.show_tick_labels {
            let _ = write!(
                s,
                r#"<text x="{px:.2}" y="{y:.2}" text-anchor="middle" font-size="{font}" fill="{axis}">{lab}</text>"#,
                y = plot.bottom() + ty.tick_outset() + TICK_LABEL_PAD + ty.tick_pt,
                font = ty.tick_pt,
                lab = svg_content(label, ty.tick_pt)
            );
        }
    }
    let y_tick_x = y_axis_x - ty.tick_outset() - TICK_LABEL_PAD;
    for (&yt, label) in y_ticks.values.iter().zip(&y_ticks.labels) {
        let (_, py) = proj.project([fig.x.min, yt]);
        let _ = write!(
            s,
            r#"<path d="M{yl:.2} {py:.2}h{tick}" stroke="{axis}" stroke-width="{width}"/>"#,
            yl = y_axis_x - ty.tick_reach(),
            tick = ty.tick_reach(),
            width = ty.axis_line_pt,
        );
        if fig.y.show_tick_labels && xf.shows_y_label_at(py - plot.top) {
            let _ = write!(
                s,
                r#"<text x="{x:.2}" y="{y:.2}" text-anchor="end" font-size="{font}" fill="{axis}">{lab}</text>"#,
                x = y_tick_x,
                y = py + ty.tick_pt * crate::rich_text::MIDDLE_TO_BASELINE,
                font = ty.tick_pt,
                lab = svg_content(label, ty.tick_pt)
            );
        }
    }
    if fig.y.show_tick_labels
        && let Some(multiplier) = y_ticks.multiplier()
    {
        let _ = write!(
            s,
            r#"<text x="{x:.2}" y="{y:.2}" text-anchor="start" font-size="{font}" fill="{axis}">{label}</text>"#,
            x = if xf.y_multiplier_inside {
                y_axis_x + ty.tick_length_pt + TICK_LABEL_PAD
            } else {
                y_axis_x
            },
            y = if xf.y_multiplier_inside {
                plot.top + TICK_LABEL_PAD + ty.tick_pt
            } else {
                plot.top - TICK_LABEL_PAD
            },
            font = ty.tick_pt,
            label = svg_content(&multiplier, ty.tick_pt),
        );
    }
    if xf.show_tick_labels
        && let Some(multiplier) = x_ticks.multiplier()
    {
        let _ = write!(
            s,
            r#"<text x="{x:.2}" y="{y:.2}" text-anchor="end" font-size="{font}" fill="{axis}">{label}</text>"#,
            x = plot.right(),
            y = x_lane_bottom - ty.tick_pt * crate::rich_text::BASELINE_TO_BOTTOM,
            font = ty.tick_pt,
            label = svg_content(&multiplier, ty.tick_pt),
        );
    }

    if !hidden_frame && xf.show_label {
        let multiplier_clearance = if xf.show_tick_labels {
            x_ticks.multiplier_clearance(ty.tick_pt)
        } else {
            0.0
        };
        let _ = write!(
            s,
            r#"<text x="{cx:.2}" y="{y:.2}" text-anchor="middle" font-size="{font}" fill="{axis}">{lab}</text>"#,
            cx = (plot.left + plot.right()) / 2.0,
            y = x_lane_bottom
                - multiplier_clearance
                - crate::rich_text::above_bottom(&fig.x.label, ty.label_pt),
            font = ty.label_pt,
            lab = svg_content(&fig.x.label, ty.label_pt),
        );
    }
    if !hidden_frame && fig.y.show_label {
        // The rotated glyph tops face the outer edge, so the lane's left edge
        // is the line top and the baseline sits on its tick side.
        let _ = write!(
            s,
            r#"<text transform="translate({x:.2},{cy:.2}) rotate(-90)" text-anchor="middle" font-size="{font}" fill="{axis}">{lab}</text>"#,
            x = y_title_left + crate::rich_text::below_top(&fig.y.label, ty.label_pt),
            cy = y_title_cy,
            font = ty.label_pt,
            lab = svg_content(&fig.y.label, ty.label_pt),
        );
    }

    let _ = write!(
        s,
        r#"<clipPath id="{clip_id}"><rect x="{l:.2}" y="{t:.2}" width="{pw:.2}" height="{ph:.2}"/></clipPath>"#,
        l = plot.left,
        t = plot.top,
        pw = plot.width,
        ph = plot.height,
    );
    let _ = write!(s, r#"<g clip-path="url(#{clip_id})">"#);
    if let Some(grid) = &fig.heatmap {
        let _ = write!(s, r#"<g shape-rendering="crispEdges">"#);
        for (cell, color) in heatmap_cells(&proj, grid) {
            let _ = write!(
                s,
                r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="{col}"/>"#,
                x = cell.left,
                y = cell.top,
                w = cell.width,
                h = cell.height,
                col = color.to_hex(),
            );
        }
        let _ = write!(s, "</g>");
    }
    for poly in &fig.polygons {
        let Some(outline) = polygon_outline(&proj, poly) else {
            continue;
        };
        let mut pts = String::new();
        for &(px, py) in &outline {
            let _ = write!(pts, "{px:.2},{py:.2} ");
        }
        let opacity = if poly.opacity < 1.0 {
            format!(r#" fill-opacity="{:.3}""#, poly.opacity)
        } else {
            String::new()
        };
        let stroke = crate::shape_stroke::shape_stroke(
            &outline,
            poly.stroke,
            poly.opacity,
            fig.background,
            1.0,
        );
        let stroke = match stroke {
            Some((c, w)) => format!(r#" stroke="{}" stroke-width="{w:.2}""#, c.to_hex()),
            None => String::new(),
        };
        let _ = write!(
            s,
            r#"<polygon points="{pts}" fill="{col}"{opacity}{stroke}/>"#,
            col = poly.fill.to_hex(),
        );
        if let Some(hatch) = poly.hatch {
            crate::hatch::write_svg(s, &outline, hatch, plot);
        }
    }
    for annotation in &fig.range_annotations {
        let Some((x0, x1)) = crate::range_annotation_xs(&proj, annotation) else {
            continue;
        };
        let left = x0.min(x1);
        let right = x0.max(x1);
        let width = (x1 - x0).abs();
        let _ = write!(
            s,
            r#"<rect class="range-annotation" x="{left:.2}" y="{top:.2}" width="{width:.2}" height="{height:.2}" fill="{color}" fill-opacity="{opacity:.3}" stroke="{color}" stroke-width="{stroke:.2}"/>"#,
            top = plot.top,
            height = plot.height,
            color = annotation.color.to_hex(),
            opacity = annotation.fill_opacity.clamp(0.0, 1.0),
            stroke = annotation.width,
        );
        let Some(label) = range_label_layout(
            plot,
            left,
            right,
            fig.typography.tick_pt,
            &annotation.label,
            annotation.label_position,
        ) else {
            continue;
        };
        let _ = write!(
            s,
            r#"<text class="range-annotation-label" x="{x:.2}" y="{y:.2}" text-anchor="{anchor}" font-size="{font:.2}" fill="{color}">{label}</text>"#,
            x = label.x,
            y = label.top + fig.typography.tick_pt,
            anchor = match label.anchor {
                TextAnchor::Left => "start",
                TextAnchor::Center => "middle",
                TextAnchor::Right => "end",
            },
            font = fig.typography.tick_pt,
            color = annotation.color.to_hex(),
            label = svg_content(&label.text, fig.typography.tick_pt),
        );
    }
    let labels = crate::label_layout(fig, plot, 1.0);
    crate::reference_lines::write_svg(s, fig, plot, &labels);
    for contour in &fig.contours {
        let mut path = String::new();
        for seg in &contour.segments {
            let (ax, ay) = proj.project(seg[0]);
            let (bx, by) = proj.project(seg[1]);
            let _ = write!(path, "M{ax:.2} {ay:.2}L{bx:.2} {by:.2}");
        }
        let _ = write!(
            s,
            r#"<path d="{path}" fill="none" stroke="{col}" stroke-width="{w}"/>"#,
            col = contour.color.to_hex(),
            w = contour.width,
        );
    }
    write_error_bars(s, fig, &proj, false);
    for (index, series) in fig.series_in_paint_order() {
        curves::write(s, &proj, series, index, lod.as_deref_mut());
    }
    write_error_bars(s, fig, &proj, true);
    for curve in integral::layout(fig, plot, 1.0) {
        let mut points = String::new();
        for (x, y) in curve.points {
            let _ = write!(points, "{x:.2},{y:.2} ");
        }
        let _ = write!(
            s,
            r#"<polyline class="integral-curve" points="{points}" fill="none" stroke="{color}" stroke-width="{width}"/>"#,
            color = curve.color.to_hex(),
            width = curve.width,
        );
        let _ = write!(
            s,
            r#"<text class="integral-label" x="{x:.2}" y="{y:.2}" text-anchor="middle" dominant-baseline="middle" font-size="{font:.2}" fill="{color}" transform="rotate(-90 {x:.2} {y:.2})">{text}</text>"#,
            x = curve.label.position.0,
            y = curve.label.position.1,
            font = curve.label.font_size,
            color = curve.label.color.to_hex(),
            text = svg_content(&curve.label.text, curve.label.font_size),
        );
    }
    arrows::write(s, &proj, fig);
    for (a, at) in crate::label_placement::drawn_texts(fig, plot, &labels, 1.0) {
        let anchor = match at.align {
            crate::text_place::TextAlign::Center => "middle",
            crate::text_place::TextAlign::Start => "start",
            crate::text_place::TextAlign::End => "end",
        };
        let baseline = if at.middle {
            r#" dominant-baseline="central""#
        } else {
            ""
        };
        for (line, at) in crate::text_place::lines(at, &a.text, a.size) {
            let _ = write!(
                s,
                r#"<text x="{x:.2}" y="{y:.2}" text-anchor="{anchor}"{baseline} font-size="{sz}" fill="{col}">{txt}</text>"#,
                x = at.x,
                y = at.y,
                sz = a.size,
                col = a.color.to_hex(),
                txt = svg_content(line, a.size),
            );
        }
    }
    let _ = write!(s, r#"</g>"#);
    // Spines go over the clipped data, so a bar or trace resting on an axis
    // cannot paint over half of its line.
    match fig.axis_frame {
        AxisFrame::Open => {
            let _ = write!(
                s,
                r#"<path d="M{l:.2} {b:.2}H{r:.2}M{yl:.2} {t:.2}V{b:.2}" fill="none" stroke="{axis}" stroke-width="{width}"/>"#,
                l = plot.left,
                r = plot.right(),
                yl = y_axis_x,
                t = plot.top,
                b = plot.bottom(),
                width = ty.axis_line_pt,
            );
        }
        AxisFrame::Box => {
            let _ = write!(
                s,
                r#"<rect x="{l:.2}" y="{t:.2}" width="{w:.2}" height="{h:.2}" fill="none" stroke="{axis}" stroke-width="{width}"/>"#,
                l = plot.left,
                t = plot.top,
                w = plot.width,
                h = plot.height,
                width = ty.axis_line_pt,
            );
            if (y_axis_x - plot.left).abs() > f32::EPSILON {
                let _ = write!(
                    s,
                    r#"<path d="M{yl:.2} {t:.2}V{b:.2}" stroke="{axis}" stroke-width="{width}"/>"#,
                    yl = y_axis_x,
                    t = plot.top,
                    b = plot.bottom(),
                    width = ty.axis_line_pt,
                );
            }
        }
        AxisFrame::Hidden => {}
    }
    crate::axis_break::write_svg(s, fig, plot, y_axis_x);

    if let Some(right) = crate::y2_axis::right_axis(fig, &layout, &proj, stack, 1.0) {
        crate::y2_axis::write_svg(s, fig, &right, plot);
    }

    if let (Some(trace), Some(band)) = (&fig.top_projection, proj.top_band) {
        write_projection(s, fig, trace, plot, band, true, clip_id);
    }
    if let (Some(trace), Some(band)) = (&fig.left_projection, proj.left_band) {
        write_projection(s, fig, trace, plot, band, false, clip_id);
    }

    legend::write(s, fig, plot, stack);
    color_scale::write(s, fig, plot);
    let _ = write!(s, "</g>");
}

fn write_error_bars(s: &mut String, fig: &Figure, proj: &Projector<'_>, draw_over_data: bool) {
    for error_bar in &fig.error_bars {
        if error_bar.draw_over_data != draw_over_data {
            continue;
        }
        let Some(segments) = error_bar_segments(proj, error_bar, 1.0) else {
            continue;
        };
        let mut path = String::new();
        for [start, end] in segments.as_slice() {
            let _ = write!(
                path,
                "M{:.2} {:.2}L{:.2} {:.2}",
                start.0, start.1, end.0, end.1
            );
        }
        let _ = write!(
            s,
            r#"<path class="error-bar" d="{path}" fill="none" stroke="{col}" stroke-width="{w}"/>"#,
            col = error_bar.color.to_hex(),
            w = error_bar.width,
        );
    }
}

fn write_projection(
    s: &mut String,
    fig: &Figure,
    trace: &AxisTrace,
    plot: Rect,
    band: Rect,
    along_x: bool,
    clip_id: &str,
) {
    let axis = plotium_figure::Color::AXIS.to_hex();
    let (x1, y1, x2, y2) = if along_x {
        (band.left, band.bottom(), band.right(), band.bottom())
    } else {
        (band.right(), band.top, band.right(), band.bottom())
    };
    let _ = write!(
        s,
        r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{axis}" stroke-width="0.75"/>"#,
    );
    let pts = projection_points(fig, trace, plot, band, along_x);
    if pts.len() < 2 {
        return;
    }
    let clip = format!("{clip_id}_band_{}", if along_x { "top" } else { "left" });
    let _ = write!(
        s,
        r#"<clipPath id="{clip}"><rect x="{l:.2}" y="{t:.2}" width="{w:.2}" height="{h:.2}"/></clipPath>"#,
        l = band.left,
        t = band.top,
        w = band.width,
        h = band.height,
    );
    let mut poly = String::new();
    for (x, y) in pts {
        let _ = write!(poly, "{x:.2},{y:.2} ");
    }
    let _ = write!(
        s,
        r#"<polyline points="{poly}" fill="none" stroke="{col}" stroke-width="{w}" clip-path="url(#{clip})"/>"#,
        col = trace.color.to_hex(),
        w = trace.width,
    );
}

pub(crate) fn write_panel_letter(s: &mut String, text: &str, position: [f32; 2], font_size: f32) {
    if text.trim().is_empty() {
        return;
    }
    let col = plotium_figure::Color::BLACK.to_hex();
    let _ = write!(
        s,
        r#"<text x="{x:.2}" y="{y:.2}" text-anchor="start" font-size="{font_size:.2}" font-weight="bold" fill="{col}">{txt}</text>"#,
        x = position[0],
        y = position[1] + font_size,
        txt = svg_content(text, font_size),
    );
}

use crate::rich_text::svg_content;

fn escape_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_figure::{Axis, AxisFrame, Color, ErrorBar, Figure, IntegralCurve, Series};

    #[test]
    fn exports_integral_result_curve_and_label() {
        let mut fig = Figure::new(
            "",
            Axis::new("ppm", 0.0, 2.0).reversed(true),
            Axis::new("intensity", 0.0, 1.0),
        )
        .with_series(Series::line(
            "trace",
            vec![[0.0, 1.0], [1.0, 2.0], [2.0, 1.0]],
        ));
        fig.integral_curves.push(IntegralCurve {
            start_ppm: 0.0,
            end_ppm: 2.0,
            normalized_area: 3.0,
            label: "3.000".to_owned(),
            color: Color::rgb(0x22, 0x8b, 0x57),
            width: 1.5,
            source_series: 0,
        });
        let out = export(&fig);
        assert!(out.contains("class=\"integral-curve\""));
        assert!(out.contains("class=\"integral-label\""));
        assert!(out.contains("3.000"));
        assert!(!out.contains("(ref)"));
        assert!(out.contains("rotate(-90"));
        assert!(!out.contains("integral-selection"));
    }

    #[test]
    fn foreground_error_bar_is_written_after_its_data_series() {
        let fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 2.0))
            .with_series(Series::line("bar", vec![[0.5, 0.0], [0.5, 1.0]]))
            .with_error_bar(ErrorBar::symmetric([0.5, 1.0], 0.25).over_data());
        let out = export(&fig);
        assert!(out.find("<polyline").unwrap() < out.find("class=\"error-bar\"").unwrap());
    }

    #[test]
    fn exports_contextual_ticks_and_one_axis_multiplier() {
        let fig = Figure::new(
            "",
            Axis::new("ppm", 76.5, 78.1).reversed(true),
            Axis::new("intensity", -500.0, 17_000.0),
        );
        let out = export(&fig);
        assert!(out.contains(">77.8</text>"));
        assert!(out.contains(r#"×10<tspan dy="-2.31" font-size="4.90">4</tspan>"#));
        assert!(!out.contains("e3</text>"));
        assert!(!out.contains("e4</text>"));
    }

    #[test]
    fn frame_style_is_open_for_1d_and_boxed_for_2d() {
        let open = export(&Figure::new(
            "",
            Axis::new("x", 0.0, 1.0),
            Axis::new("y", 0.0, 1.0),
        ));
        let open_rects = open.matches("<rect ").count();

        let boxed = export(
            &Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
                .with_axis_frame(AxisFrame::Box),
        );
        assert_eq!(boxed.matches("<rect ").count(), open_rects + 1);
        assert!(boxed.contains("fill=\"none\" stroke=\"#000000\""));
    }

    #[test]
    fn tiny_figure_keeps_axis_lines_after_ticks_are_dropped() {
        let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
        fig.width = 24.0;
        fig.height = 24.0;
        let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
        let layout = crate::axis_layout(&fig, fig.width, fig.height);
        assert!(layout.x_ticks.labels.is_empty() && layout.y_ticks.labels.is_empty());
        let plot = Projector::new(&fig, outer, &layout.margins).plot;
        let axis_path = format!(
            r##"<path d="M{l:.2} {b:.2}H{r:.2}M{l:.2} {t:.2}V{b:.2}" fill="none" stroke="#000000""##,
            l = plot.left,
            r = plot.right(),
            t = plot.top,
            b = plot.bottom(),
        );
        let svg = export(&fig);
        assert!(
            svg.contains(&axis_path),
            "missing open-axis path: {axis_path}"
        );
        assert_eq!(svg.matches("stroke=\"#000000\"").count(), 1);
    }
}

#[cfg(test)]
#[path = "svg_annotation_tests.rs"]
mod annotation_tests;
