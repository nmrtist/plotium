use super::*;
use plotium_figure::{Axis, ErrorBar, Series};

fn curve(points: Vec<[f64; 2]>) -> Figure {
    Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("s", points))
}

#[test]
fn automatic_log_window_uses_positive_values_padded_in_decades() {
    let range = log_auto_range([-3.0, 0.0, 1.0, 1000.0, f64::NAN]);
    assert!((range.min.log10() + 0.15).abs() < 1e-12);
    assert!((range.max.log10() - 3.15).abs() < 1e-12);
    let single = log_auto_range([100.0]);
    assert!((single.min.log10() - 1.5).abs() < 1e-12);
    assert!((single.max.log10() - 2.5).abs() < 1e-12);
    assert_eq!(log_auto_range([0.0, -1.0]), AxisRange::new(1.0, 10.0));
}

#[test]
fn only_curve_plots_offer_log_axes() {
    let line = curve(vec![[1.0, 1.0], [2.0, 4.0]]);
    assert_eq!(log_scale_unavailable(&line, true), None);
    let mut categorical = line.clone();
    categorical.x = Axis::categorical("x", vec!["a".into()]);
    assert!(log_scale_unavailable(&categorical, true).is_some());
    assert_eq!(log_scale_unavailable(&categorical, false), None);
    let mut contour = line.clone();
    contour.contours.push(plotium_figure::Contour {
        segments: vec![],
        color: plotium_figure::Color::BLACK,
        width: 1.0,
    });
    assert!(log_scale_unavailable(&contour, false).is_some());
}

#[test]
fn log_overrides_refit_the_automatic_window_and_keep_manual_ones() {
    let mut figure = curve(vec![[1.0, 0.0], [10.0, 10.0], [100.0, 100.0]]);
    figure
        .error_bars
        .push(ErrorBar::symmetric([10.0, 10.0], 9.0));
    let overrides = AxisOverrides {
        x_scale: Some(AxisScale::Log10),
        y_scale: Some(AxisScale::Log10),
        y_range: Some(AxisRange::new(2.0, 50.0)),
        ..AxisOverrides::default()
    };
    overrides.apply_to(&mut figure);
    assert_eq!(figure.x.scale, AxisScale::Log10);
    assert!(figure.x.min > 0.0 && figure.x.min < 1.0);
    assert_eq!((figure.y.min, figure.y.max), (2.0, 50.0));
    assert_eq!(figure.y.scale, AxisScale::Log10);

    let mut figure = curve(vec![[1.0, 0.0], [10.0, 10.0]]);
    figure
        .error_bars
        .push(ErrorBar::symmetric([10.0, 10.0], 9.0));
    AxisOverrides {
        y_scale: Some(AxisScale::Log10),
        ..AxisOverrides::default()
    }
    .apply_to(&mut figure);
    // The lower whisker end at 1 sets the floor, the zero sample does not.
    assert!(figure.y.min > 0.5 && figure.y.min < 1.0, "{}", figure.y.min);
    assert!(figure.y.max > 19.0, "{}", figure.y.max);
}

#[test]
fn log_navigation_moves_by_decades() {
    let full = AxisRange::new(1.0, 10_000.0);
    let zoomed = full.zoom_around_in(full, 100.0, 0.5, AxisScale::Log10);
    assert!((zoomed.min - 10.0).abs() < 1e-9 && (zoomed.max - 1000.0).abs() < 1e-7);
    let panned = zoomed.shifted_in(full, 1.0, AxisScale::Log10);
    assert!((panned.min - 100.0).abs() < 1e-7 && (panned.max - 10_000.0).abs() < 1e-5);
    // Zooming all the way out returns the stored window bit for bit.
    let mut wide = AxisRange::new(3.0, 7000.0);
    wide = wide.zoom_around_in(wide, 50.0, 4.0, AxisScale::Log10);
    assert_eq!(wide, AxisRange::new(3.0, 7000.0));
    // Linear axes are unchanged.
    let linear = AxisRange::new(0.0, 10.0);
    assert_eq!(
        linear.zoom_around_in(linear, 2.0, 0.5, AxisScale::Linear),
        linear.zoom_around(linear, 2.0, 0.5)
    );
    // A selection wider than the window in linear units but not in decades
    // keeps its log extent instead of snapping to the full window.
    let full = AxisRange::new(1.0, 1000.0);
    let selected = AxisRange::new(400.0, 1500.0).clamp_in(full, AxisScale::Log10);
    assert!((selected.max - 1000.0).abs() < 1e-9);
    assert!((selected.min - 266.666_666_666_666_7).abs() < 1e-6);
}
