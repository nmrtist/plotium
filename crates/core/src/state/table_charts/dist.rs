//! Distribution charts: histogram of one column, and per-column box / violin
//! summaries on a categorical axis (one slot per table column).

use super::super::ChartContext;
use super::super::table::{TableDataset, series_color};
use super::{empty_axes_figure, finite_values, title_of};
use plotium_analysis::statistics::{BinRule, histogram};
use plotium_figure::{Axis, Figure, Polygon};

pub(crate) fn histogram_figure(dataset: &TableDataset, ctx: &ChartContext) -> Figure {
    let plot = dataset.typed_plot_data(100_000).unwrap_or_default();
    let column = ctx
        .column
        .and_then(|column| {
            plot.series
                .iter()
                .position(|series| series.binding.value_column == column)
        })
        .unwrap_or(0);
    let Some(col) = plot.series.get(column) else {
        return empty_axes_figure(dataset, "Value", "Count");
    };
    let values = finite_values(&col.y);
    let rule = ctx.bins.map(BinRule::Count).unwrap_or(BinRule::Auto);
    let Ok(hist) = histogram(&values, rule) else {
        return empty_axes_figure(dataset, &col.name, "Count");
    };

    let x_span = hist.edges[hist.edges.len() - 1] - hist.edges[0];
    let xr = x_span.max(f64::MIN_POSITIVE);
    let max_count = hist.counts.iter().copied().max().unwrap_or(0) as f64;
    let x = Axis::new(
        col.name.clone(),
        hist.edges[0] - 0.02 * xr,
        hist.edges[hist.edges.len() - 1] + 0.02 * xr,
    );
    let y = Axis::new("Count", 0.0, max_count.max(1.0) * 1.08);
    let mut fig = Figure::new(title_of(dataset), x, y);
    let color = series_color(column);
    for (i, &count) in hist.counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        fig.polygons.push(
            Polygon::rect(
                String::new(),
                hist.edges[i],
                hist.edges[i + 1],
                0.0,
                count as f64,
                color,
            )
            // A background-colored hairline separates adjacent bins.
            .with_stroke(fig.background, 0.5),
        );
    }
    fig
}

pub(crate) fn box_figure(dataset: &TableDataset, _ctx: &ChartContext) -> Figure {
    let plot = dataset.typed_plot_data(100_000).unwrap_or_default();
    if plot.series.is_empty() {
        return empty_axes_figure(dataset, "", "Value");
    }
    let names = plot.series.iter().map(|c| c.name.clone()).collect();
    let mut fig = Figure::new(
        title_of(dataset),
        Axis::categorical("", names),
        Axis::new("Value", 0.0, 1.0),
    );
    let (mut ylo, mut yhi) = (f64::INFINITY, f64::NEG_INFINITY);

    for (j, col) in plot.series.iter().enumerate() {
        let values = finite_values(&col.y);
        let Some(marks) = super::marks::box_marks(&values, j as f64, 0.3, series_color(j), "", 1.0)
        else {
            continue;
        };
        ylo = ylo.min(marks.y_range[0]);
        yhi = yhi.max(marks.y_range[1]);
        fig.polygons.extend(marks.polygons);
        fig.series.extend(marks.series);
    }

    set_padded_y(&mut fig, ylo, yhi);
    fig
}

pub(crate) fn violin_figure(dataset: &TableDataset, _ctx: &ChartContext) -> Figure {
    let plot = dataset.typed_plot_data(100_000).unwrap_or_default();
    if plot.series.is_empty() {
        return empty_axes_figure(dataset, "", "Value");
    }
    let names = plot.series.iter().map(|c| c.name.clone()).collect();
    let mut fig = Figure::new(
        title_of(dataset),
        Axis::categorical("", names),
        Axis::new("Value", 0.0, 1.0),
    );
    let (mut ylo, mut yhi) = (f64::INFINITY, f64::NEG_INFINITY);

    for (j, col) in plot.series.iter().enumerate() {
        let values = finite_values(&col.y);
        let Some(marks) = super::marks::violin_marks(
            &values,
            j as f64,
            0.42,
            series_color(j),
            "",
            plotium_figure::DEFAULT_DATA_LINE_WIDTH_PT,
        ) else {
            continue;
        };
        ylo = ylo.min(marks.y_range[0]);
        yhi = yhi.max(marks.y_range[1]);
        fig.polygons.extend(marks.polygons);
        fig.series.extend(marks.series);
    }

    set_padded_y(&mut fig, ylo, yhi);
    fig
}

fn set_padded_y(fig: &mut Figure, ylo: f64, yhi: f64) {
    let (ylo, yhi) = if ylo.is_finite() && yhi.is_finite() {
        (ylo, yhi)
    } else {
        (0.0, 1.0)
    };
    let span = yhi - ylo;
    // A constant sample (all values equal) has zero span; pad relative to the
    // value's own magnitude so the mark sits centered on a readable axis
    // instead of at the bottom of a fallback unit range.
    let pad = if span > 0.0 {
        0.05 * span
    } else {
        (yhi.abs() * 0.05).max(0.5)
    };
    fig.y.min = ylo - pad;
    fig.y.max = yhi + pad;
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        ChartContext, FloatSeries, TableDataset, materialized_float_series_table,
    };

    fn table_with(values: Vec<f64>) -> TableDataset {
        let n = values.len();
        materialized_float_series_table(
            (
                "index".into(),
                "".into(),
                (0..n).map(|i| Some(i as f64)).collect(),
            ),
            vec![FloatSeries {
                name: "sample".to_owned(),
                unit: String::new(),
                values: values.into_iter().map(Some).collect(),
                uncertainty: None,
                fit: None,
            }],
            "plotium.test.distribution-table.v1",
        )
        .unwrap()
    }

    #[test]
    fn histogram_bins_the_selected_column_and_labels_count() {
        let dataset = table_with((0..100).map(|i| i as f64).collect());
        let fig = super::histogram_figure(&dataset, &ChartContext::default());
        assert!(!fig.polygons.is_empty());
        assert_eq!(fig.y.label, "Count");
        assert_eq!(fig.x.label, "sample");
        // Every bin footprint sits inside the padded x range.
        assert!(fig.polygons.iter().all(|p| {
            p.points
                .iter()
                .all(|pt| pt[0] >= fig.x.min && pt[0] <= fig.x.max)
        }));
    }

    #[test]
    fn histogram_respects_a_manual_bin_count() {
        let dataset = table_with((0..100).map(|i| i as f64).collect());
        let ctx = ChartContext {
            bins: Some(4),
            ..ChartContext::default()
        };
        let fig = super::histogram_figure(&dataset, &ctx);
        assert_eq!(fig.polygons.len(), 4);
    }

    #[test]
    fn box_figure_draws_body_median_whiskers_and_outliers() {
        // 1..9 plus a far outlier at 100.
        let mut values: Vec<f64> = (1..=9).map(|i| i as f64).collect();
        values.push(100.0);
        let fig = super::box_figure(&table_with(values), &ChartContext::default());
        assert_eq!(fig.polygons.len(), 1, "one box body");
        // Median + 2 whisker stems + 2 caps + outlier points.
        assert_eq!(fig.series.len(), 6);
        let outliers = fig.series.last().unwrap();
        assert_eq!(outliers.points, vec![[0.0, 100.0]]);
        assert_eq!(
            fig.x.categories.as_deref(),
            Some(&["sample".to_owned()][..])
        );
    }

    #[test]
    fn violin_emits_convex_strips_and_inner_quartile_bar() {
        let values: Vec<f64> = (0..60).map(|i| (i % 10) as f64).collect();
        let fig = super::violin_figure(&table_with(values), &ChartContext::default());
        // 119 density strips + 1 quartile bar.
        assert_eq!(fig.polygons.len(), 120);
        assert!(fig.polygons.iter().all(|p| p.points.len() >= 4));
        // Strips stay within the slot half-width.
        assert!(
            fig.polygons
                .iter()
                .all(|p| p.points.iter().all(|pt| pt[0].abs() <= 0.421))
        );
    }

    #[test]
    fn degenerate_columns_fall_back_without_panicking() {
        let constant = table_with(vec![5.0; 8]);
        let fig = super::violin_figure(&constant, &ChartContext::default());
        // KDE refuses a zero-variance sample; raw points are drawn instead.
        assert!(fig.polygons.is_empty());
        assert_eq!(fig.series.len(), 1);
        // The constant value sits inside a visibly padded axis, not at the
        // bottom of a fallback unit range.
        assert!(fig.y.min < 5.0 && fig.y.max > 5.0);
        assert!(fig.y.max - fig.y.min >= 0.5);

        let box_fig = super::box_figure(&constant, &ChartContext::default());
        assert!(box_fig.y.min < 5.0 && box_fig.y.max > 5.0);

        let empty = table_with(Vec::new());
        assert!(
            super::box_figure(&empty, &ChartContext::default())
                .polygons
                .is_empty()
        );
        assert!(
            super::histogram_figure(&empty, &ChartContext::default())
                .polygons
                .is_empty()
        );
    }
}
