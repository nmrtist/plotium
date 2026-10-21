use super::*;
use crate::theme::Theme;
use plotium_figure::{
    Color, ColorSource, PositiveFiniteF32, SeriesEncoding, SeriesKind, TickDirection,
};

fn first_line(app: &PlotiumApp) -> &plotium_figure::LineEncoding {
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    match &plot.binding.series[0].encoding {
        SeriesEncoding::Line(line) => line,
        _ => panic!("sample plot draws a line series"),
    }
}

fn first_line_mut(app: &mut PlotiumApp) -> &mut plotium_figure::LineEncoding {
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    match &mut plot.binding.series[0].encoding {
        SeriesEncoding::Line(line) => line,
        _ => panic!("sample plot draws a line series"),
    }
}

#[test]
fn nature_theme_sets_axis_furniture_lines_and_markers_undoably() {
    let mut app = sample_app();
    first_line_mut(&mut app).appearance.marker_edge_width = PositiveFiniteF32::new(0.5);
    let before_typography = app.doc.style_library.figure_typography;
    let before_line = first_line(&app).clone();

    app.apply_theme(&Theme::by_id("nature").unwrap());

    let typography = app.doc.style_library.figure_typography;
    assert_eq!(typography.tick_direction, TickDirection::Out);
    assert_eq!(typography.axis_line_pt, 0.6);
    assert_eq!(typography.tick_pt, 7.0);
    let line = first_line(&app);
    assert_eq!(line.width.get(), 0.75);
    // A plain line draws no markers, so its marker size is left alone.
    assert_eq!(line.appearance.marker_size, None);
    assert_eq!(line.appearance.marker_edge_width, None);
    assert_eq!(line.color, ColorSource::Explicit(Color::rgb(0, 0, 0)));
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(plot.figure().typography, typography);

    app.undo();
    assert_eq!(app.doc.style_library.figure_typography, before_typography);
    assert_eq!(first_line(&app), &before_line);
}

#[test]
fn themes_without_line_settings_keep_series_width() {
    let mut app = sample_app();
    app.apply_theme(&Theme::by_id("nature").unwrap());
    app.apply_theme(&Theme::by_id("publication").unwrap());
    assert_eq!(first_line(&app).width.get(), 0.75);
}

#[test]
fn nature_theme_sizes_markers_but_not_point_widths() {
    let mut app = sample_app();
    first_line_mut(&mut app).kind = SeriesKind::Points;
    let before_width = first_line(&app).width;
    app.apply_theme(&Theme::by_id("nature").unwrap());
    let line = first_line(&app);
    assert_eq!(line.appearance.marker_size.map(|s| s.get()), Some(3.0));
    assert_eq!(line.width, before_width);
}
