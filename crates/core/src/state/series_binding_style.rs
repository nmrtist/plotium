//! A series binding's own presentation, stamped onto the curves it builds.
use super::*;
use plotium_figure::Figure;

impl PlotiumApp {
    pub(super) fn apply_series_binding_style(&self, figure: &mut Figure, binding: &SeriesBinding) {
        let plotium_figure::SeriesEncoding::Line(line) = &binding.encoding else {
            return;
        };
        let color = line.color.resolve();
        let x_shift = line.x_shift.get();
        figure.x.min += x_shift;
        figure.x.max += x_shift;
        let semantic_colors = figure.series_colors_are_semantic;
        for series in &mut figure.series {
            series.origin = Some(plotium_figure::SeriesOrigin(binding.id.get()));
            if let Some(label) = &binding.label {
                series.name = label.clone();
            }
            if !semantic_colors {
                series.color = color;
            }
            series.width = line.width.get();
            series.appearance = line.appearance;
            if semantic_colors {
                // A single marker colour would hide the colours that carry meaning.
                series.appearance.marker_fill = None;
                series.appearance.marker_edge_color = None;
            }
            series.display_baseline *= line.scale;
            for point in &mut series.points {
                point[0] += x_shift;
                point[1] *= line.scale;
            }
        }
        for error_bar in &mut figure.error_bars {
            error_bar.y_axis = line.appearance.y_axis;
            if !semantic_colors {
                error_bar.color = color;
            }
            error_bar.width = line.width.get();
            error_bar.display_baseline *= line.scale;
            error_bar.center[0] += x_shift;
            error_bar.center[1] *= line.scale;
            error_bar.negative *= line.scale.abs();
            error_bar.positive *= line.scale.abs();
        }
        for curve in &mut figure.integral_curves {
            curve.start_ppm += x_shift;
            curve.end_ppm += x_shift;
        }
        for polygon in &mut figure.polygons {
            for point in &mut polygon.points {
                point[0] += x_shift;
            }
        }
        for annotation in &mut figure.annotations {
            annotation.at[0] += x_shift;
        }
        for range in &mut figure.range_annotations {
            range.x0 += x_shift;
            range.x1 += x_shift;
        }
        if !semantic_colors
            && figure.heatmap.is_none()
            && figure.axis_frame != plotium_figure::AxisFrame::Hidden
        {
            let background = figure.background;
            for polygon in &mut figure.polygons {
                polygon.fill = color;
                if let Some((stroke, width)) = &mut polygon.stroke
                    && *stroke != background
                {
                    *stroke = color;
                    *width = line.width.get();
                }
            }
        }
    }
}
