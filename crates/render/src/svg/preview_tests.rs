use super::*;
use crate::{DocumentItem, DocumentObject, Rect};
use plotium_figure::{Axis, AxisScale, Color, Figure};

fn page(figure: &Figure) -> Document<'_> {
    Document {
        width: 400.0,
        height: 300.0,
        background: Color::rgb(255, 255, 255),
        items: vec![DocumentItem::Plot(DocumentObject {
            id: "plot".into(),
            frame: Rect::new(0.0, 0.0, 400.0, 300.0),
            figure,
            geometry_generation: None,
            visible: true,
            title: None,
            y_title_clearance: Default::default(),
            stack: None,
        })],
    }
}

fn dense_line(count: usize) -> Vec<[f64; 2]> {
    (0..count)
        .map(|i| {
            let x = i as f64 / count as f64;
            [x, (x * 400.0).sin() + 2.0]
        })
        .collect()
}

#[test]
fn dense_line_is_enveloped_to_preview_columns() {
    let mut figure = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 4.0));
    let mut series = Series::line("dense", dense_line(620_000));
    series.origin = Some(SeriesOrigin(7));
    figure.series.push(series);
    let document = page(&figure);

    let full = crate::svg::export_document(&document);
    let preview = export_document_preview(&document, 2.0);

    assert!(full.len() > 8 * 1024 * 1024, "{}", full.len());
    assert!(preview.svg.len() < 256 * 1024, "{}", preview.svg.len());
    let [record] = preview.downsampled.as_slice() else {
        panic!("{:?}", preview.downsampled);
    };
    assert_eq!((record.item, record.series), (0, 0));
    assert_eq!(record.origin, Some(SeriesOrigin(7)));
    assert_eq!(record.source_points, 620_000);
    // At most a min/max pair per envelope column: four per pixel of a plot
    // narrower than the 400 pt page at 2 px/pt.
    assert!(
        record.drawn_points <= 2 * 4 * 800 + 2,
        "{}",
        record.drawn_points
    );
}

#[test]
fn log_axis_line_keeps_its_envelope() {
    let points: Vec<[f64; 2]> = (1..=200_000)
        .map(|i| {
            [
                f64::from(i) * 0.05,
                10f64.powf((f64::from(i) * 0.01).sin() * 3.0),
            ]
        })
        .collect();
    let mut x = Axis::new("", 0.05, 10_000.0);
    x.scale = AxisScale::Log10;
    let mut y = Axis::new("", 1e-3, 1e3);
    y.scale = AxisScale::Log10;
    let mut figure = Figure::new("", x, y);
    figure.series.push(Series::line("log", points));
    let preview = export_document_preview(&page(&figure), 1.0);

    let [record] = preview.downsampled.as_slice() else {
        panic!("{:?}", preview.downsampled);
    };
    // The 2,048-column floor applies to a narrow plot at 1 px/pt.
    assert!(
        record.drawn_points <= 2 * 2_048 + 2,
        "{}",
        record.drawn_points
    );
    assert!(preview.svg.contains("<polyline"));
}

#[test]
fn curves_within_budget_match_the_full_export() {
    let mut figure = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 4.0));
    figure.series.push(Series::line("short", dense_line(300)));
    figure.series.push(Series::points("few", dense_line(200)));
    let document = page(&figure);

    let preview = export_document_preview(&document, 2.0);
    assert!(preview.downsampled.is_empty());
    assert_eq!(preview.svg, crate::svg::export_document(&document));
}

#[test]
fn coincident_markers_are_drawn_once_per_pixel() {
    let points: Vec<[f64; 2]> = (0..100_000)
        .map(|i| [f64::from(i % 50) / 50.0, f64::from(i % 40) / 10.0])
        .collect();
    let mut figure = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 4.0));
    figure.series.push(Series::points("scatter", points));
    let preview = export_document_preview(&page(&figure), 1.0);

    let [record] = preview.downsampled.as_slice() else {
        panic!("{:?}", preview.downsampled);
    };
    assert_eq!(record.source_points, 100_000);
    assert_eq!(record.drawn_points, 200);
    assert_eq!(preview.svg.matches("<circle").count(), 200);
}

#[test]
fn gaps_stay_broken_when_a_line_is_thinned() {
    let mut points = dense_line(300_000);
    for gap in [50_000, 50_001, 150_000, 200_007] {
        points[gap][1] = f64::NAN;
    }
    let mut figure = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 4.0));
    figure.series.push(Series::line("gaps", points));
    let document = page(&figure);

    let full = crate::svg::export_document(&document);
    let preview = export_document_preview(&document, 1.0);
    assert_eq!(preview.downsampled.len(), 1);
    assert_eq!(full.matches("<polyline").count(), 4);
    assert_eq!(preview.svg.matches("<polyline").count(), 4);
}

#[test]
fn non_monotonic_x_keeps_samples_inside_the_axis_range() {
    // A loop out to x = 2 and back, viewed on 0..1.
    let points: Vec<[f64; 2]> = (0..200_000)
        .map(|i| {
            let t = f64::from(i) / 100_000.0;
            [if t <= 1.0 { 2.0 * t } else { 4.0 - 2.0 * t }, t]
        })
        .collect();
    let drawn = crate::line_lod::line_points(&points, 0.0, 1.0, 100);
    assert!(drawn.pooled);
    // The return pass through the visible range (y > 1.5) survives clipping.
    assert!(drawn.points.iter().any(|p| p[0] < 1.0 && p[1] > 1.5));
}

#[test]
fn unthinned_curves_with_missing_points_are_not_reported() {
    let mut points = dense_line(2_000);
    points[3][1] = f64::NAN;
    let mut figure = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 4.0));
    figure
        .series
        .push(Series::points("scatter", points[..100].to_vec()));
    // Over the column budget but entirely outside the x range: clipped, not pooled.
    figure.series.push(Series::line(
        "off-axis",
        points.iter().map(|p| [p[0] + 5.0, p[1]]).collect(),
    ));
    let document = page(&figure);
    let preview = export_document_preview(&document, 1.0);
    assert!(preview.downsampled.is_empty(), "{:?}", preview.downsampled);
    assert_eq!(preview.svg, crate::svg::export_document(&document));
}

#[test]
fn a_zoomed_window_keeps_its_visible_points_without_thinning() {
    let mut figure = Figure::new("", Axis::new("", 0.5, 0.501), Axis::new("", 0.0, 4.0));
    figure
        .series
        .push(Series::line("dense", dense_line(2_000_000)));
    let preview = export_document_preview(&page(&figure), 2.0);

    assert!(preview.downsampled.is_empty(), "{:?}", preview.downsampled);
    // About 2,000 visible samples plus one neighbour each side.
    let vertices = preview.svg.matches(',').count();
    assert!((2_000..2_100).contains(&vertices), "{vertices}");
    assert!(preview.svg.len() < 256 * 1024, "{}", preview.svg.len());
}

#[test]
fn thinning_keeps_four_columns_per_pixel_above_a_floor() {
    let mut figure = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 4.0));
    figure
        .series
        .push(Series::line("dense", dense_line(620_000)));
    let document = page(&figure);
    let drawn = |pixels_per_pt| {
        export_document_preview(&document, pixels_per_pt).downsampled[0].drawn_points
    };

    // A small raster still gets the 2,048-column floor.
    assert!(drawn(0.1) > 2 * 2_000, "{}", drawn(0.1));
    // Above the floor the envelope scales with the raster width.
    let (two, four) = (drawn(2.0), drawn(4.0));
    assert!(two > 2 * 2_048, "{two}");
    assert!(four as f64 > 1.8 * two as f64, "{two} {four}");
}
