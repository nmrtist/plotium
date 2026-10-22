use super::*;
use crate::theme::{MarkerPaint, Theme};
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

#[test]
fn canvas_style_capture_restores_the_canvas_look() {
    let mut app = sample_app();
    {
        let line = first_line_mut(&mut app);
        line.kind = SeriesKind::Points;
        line.color = ColorSource::Explicit(Color::rgb(0xe6, 0xaf, 0x2d));
        line.appearance.marker_size = PositiveFiniteF32::new(3.4);
        line.appearance.marker_fill = Some(Color::rgb(0xf2, 0xd7, 0x97));
        line.appearance.marker_edge_color = Some(Color::rgb(0x40, 0x40, 0x40));
        line.appearance.marker_edge_width = PositiveFiniteF32::new(0.5);
        line.appearance.marker_hollow = true;
    }
    app.doc.style_library.figure_typography.tick_pt = 5.0;
    app.doc.style_library.figure_typography.tick_direction = TickDirection::In;
    let styled = first_line(&app).clone();

    let captured = app.theme_from_canvas(0, "Lab").unwrap();
    assert_eq!(captured.name, "Lab");
    assert_eq!(captured.trace_palette, vec![Color::rgb(0xe6, 0xaf, 0x2d)]);
    assert_eq!(captured.marker_size_pt, Some(3.4));

    app.apply_theme(&Theme::by_id("nature").unwrap());
    assert_ne!(first_line(&app), &styled);
    app.apply_theme(&captured);
    assert_eq!(first_line(&app), &styled);
    let typography = app.doc.style_library.figure_typography;
    assert_eq!(typography.tick_pt, 5.0);
    assert_eq!(typography.tick_direction, TickDirection::In);
}

#[test]
fn canvas_without_plots_captures_the_publication_palette() {
    let mut app = sample_app();
    app.doc.canvases[0].objects.clear();
    let captured = app.theme_from_canvas(0, "Empty").unwrap();
    assert_eq!(
        captured.trace_palette,
        Theme::by_id("publication").unwrap().trace_palette
    );
    assert!(captured.marker_palette.is_empty());
    assert!(app.theme_from_canvas(5, "Missing").is_none());
}

#[test]
fn nature_theme_hints_at_column_widths_without_resizing() {
    let mut app = sample_app();
    app.doc.canvases[0].size_mm = [100.04, 80.0];
    app.apply_theme(&Theme::by_id("nature").unwrap());
    assert_eq!(
        app.session.status,
        "Applied the Nature theme. The Nature theme is sized for the 89 / 136 / 183 mm \
         column presets; this canvas is 100 mm wide."
    );
    assert_eq!(app.doc.canvases[0].size_mm, [100.04, 80.0]);

    // Journal presets match on width alone.
    app.doc.canvases[0].size_mm = [136.0, 200.0];
    app.apply_theme(&Theme::by_id("nature").unwrap());
    assert_eq!(app.session.status, "Applied the Nature theme.");

    app.doc.canvases[0].size_mm = [120.0, 80.0];
    app.apply_theme(&Theme::by_id("publication").unwrap());
    assert_eq!(app.session.status, "Applied the Publication theme.");
}

#[test]
fn rethemed_curves_drop_leftover_marker_colours_undoably() {
    let mut app = sample_app();
    {
        let line = first_line_mut(&mut app);
        line.kind = SeriesKind::Points;
        line.color = ColorSource::Explicit(Color::rgb(0xe6, 0x9f, 0x00));
        line.appearance.marker_fill = Some(Color::rgb(0xf2, 0xd7, 0x97));
        line.appearance.marker_edge_color = Some(Color::rgb(0x40, 0x40, 0x40));
    }
    let before = first_line(&app).clone();
    let publication = Theme::by_id("publication").unwrap();

    app.apply_theme(&publication);

    let line = first_line(&app);
    assert_eq!(
        line.color,
        ColorSource::Explicit(publication.trace_palette[0])
    );
    assert_eq!(line.appearance.marker_fill, None);
    assert_eq!(line.appearance.marker_edge_color, None);
    app.undo();
    assert_eq!(first_line(&app), &before);
}

#[test]
fn locked_curves_keep_their_colours_and_marker_paint() {
    let mut app = sample_app();
    let chosen = Color::rgb(0xe6, 0x9f, 0x00);
    let fill = Color::rgb(0xf2, 0xd7, 0x97);
    let edge = Color::rgb(0x40, 0x40, 0x40);
    {
        let line = first_line_mut(&mut app);
        line.kind = SeriesKind::Points;
        line.color = ColorSource::Explicit(chosen);
        line.appearance.marker_fill = Some(fill);
        line.appearance.marker_edge_color = Some(edge);
        line.appearance.marker_hollow = true;
        line.color_locked = true;
    }
    let mut captured = app.theme_from_canvas(0, "Lab").unwrap();
    captured.trace_palette = vec![Color::rgb(0, 0, 0)];
    captured.marker_palette = vec![Some(MarkerPaint::default())];

    app.apply_theme(&Theme::by_id("nature").unwrap());
    // Sizing still follows the theme; the colours do not.
    let line = first_line(&app);
    assert_eq!(line.appearance.marker_size.map(|s| s.get()), Some(3.0));
    assert_eq!(line.color, ColorSource::Explicit(chosen));
    // A style with its own marker paint does not repaint a locked curve either.
    app.apply_theme(&captured);

    let line = first_line(&app);
    assert_eq!(line.color, ColorSource::Explicit(chosen));
    assert_eq!(line.appearance.marker_fill, Some(fill));
    assert_eq!(line.appearance.marker_edge_color, Some(edge));
    assert!(line.appearance.marker_hollow);
}
