use super::*;
use crate::label_layout;
use plotium_figure::{Axis, Series, TraceLabel, TraceLabels, YAxisSide};

const PLOT: Rect = Rect {
    left: 40.0,
    top: 10.0,
    width: 200.0,
    height: 150.0,
};

/// Flat traces one unit apart, with `peaks` x positions rising through each.
fn stack(names: &[&str], peaks: &[f64], side: TraceLabelSide) -> Figure {
    // Room above the last trace, as its peaks leave in a real stack.
    let top = names.len() as f64 + 1.0;
    let mut fig = Figure::new("", Axis::new("x", 0.0, 100.0), Axis::new("y", 0.0, top));
    for (index, name) in names.iter().enumerate() {
        let level = index as f64 + 0.1;
        let points = (0..=1000).map(|step| {
            let x = f64::from(step) / 10.0;
            let peak = peaks.iter().any(|peak| (x - peak).abs() < 0.3);
            [x, if peak { level + 0.8 } else { level }]
        });
        fig.series.push(Series::line(*name, points.collect()));
    }
    fig.trace_labels = Some(TraceLabels {
        side,
        traces: (0..names.len())
            .map(|series| TraceLabel {
                text: names[series].to_owned(),
                series,
            })
            .collect(),
    });
    fig
}

const NAMES: [&str; 4] = ["sample_a.xy", "sample_b.xy", "sample_c.xy", "sample_d.xy"];

#[test]
fn names_stand_on_their_traces_in_the_trace_colour() {
    let mut fig = stack(&NAMES, &[], TraceLabelSide::Right);
    fig.series[2].color = Color::rgb(200, 10, 10);
    let texts = place(&fig, PLOT).texts;
    assert_eq!(texts.len(), 4);
    for (text, name) in texts.iter().zip(NAMES) {
        let index = NAMES.iter().position(|n| *n == name).unwrap();
        assert_eq!(text.annotation.text, name);
        let line = project(&fig, PLOT, [50.0, index as f64 + 0.1], YAxisSide::Left).1;
        assert!(text.bounds().bottom() <= line, "{name} sits above its line");
        assert!(line - text.bounds().bottom() < 3.0, "{name} stays near it");
    }
    assert_eq!(texts[2].annotation.color, Color::rgb(200, 10, 10));
}

#[test]
fn a_fixed_side_holds_its_end_of_the_plot() {
    let right = place(&stack(&NAMES, &[], TraceLabelSide::Right), PLOT).texts;
    let left = place(&stack(&NAMES, &[], TraceLabelSide::Left), PLOT).texts;
    for text in &right {
        assert_eq!(text.at.align, TextAlign::End);
        assert!((text.bounds().right() - (PLOT.right() - GAP_PT)).abs() < 1e-3);
    }
    for text in &left {
        assert_eq!(text.at.align, TextAlign::Start);
        assert!((text.bounds().left - (PLOT.left + GAP_PT)).abs() < 1e-3);
    }
}

#[test]
fn the_automatic_side_prefers_the_right_end_of_a_clear_plot() {
    let texts = place(&stack(&NAMES, &[], TraceLabelSide::Auto), PLOT).texts;
    assert!(texts.iter().all(|text| text.at.align == TextAlign::End));
}

#[test]
fn the_automatic_side_leaves_peaks_clear() {
    // Peaks at the right end, as in a diffraction pattern, and one by the left.
    let fig = stack(
        &NAMES,
        &[3.0, 60.0, 72.0, 85.0, 90.0, 95.0],
        TraceLabelSide::Auto,
    );
    let texts = place(&fig, PLOT).texts;
    let clear_of = |text: &TraceText, peak: f64| {
        let x = project(&fig, PLOT, [peak, 0.0], YAxisSide::Left).0;
        let bounds = text.bounds();
        x < bounds.left || x > bounds.right()
    };
    for text in &texts {
        for peak in [3.0, 60.0, 72.0, 85.0, 90.0, 95.0] {
            assert!(
                clear_of(text, peak),
                "{} crosses {peak}",
                text.annotation.text
            );
        }
    }
}

#[test]
fn tight_stacks_shrink_then_skip_names() {
    let names: Vec<String> = (0..10).map(|index| format!("t{index}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let fig = stack(&names, &[], TraceLabelSide::Right);
    let roomy = place(&fig, PLOT).texts;
    assert_eq!(roomy.len(), 10);
    assert_eq!(roomy[0].annotation.size, fig.typography.legend_pt);

    let squat = Rect {
        height: 70.0,
        ..PLOT
    };
    let shrunk = place(&fig, squat).texts;
    assert_eq!(shrunk.len(), 10);
    assert!(shrunk[0].annotation.size < fig.typography.legend_pt);

    let flat = Rect {
        height: 20.0,
        ..PLOT
    };
    let TracePlacement {
        texts: thinned,
        omitted,
    } = place(&fig, flat);
    assert!(thinned.len() >= 2);
    assert_eq!(thinned.len() + omitted.len(), 10);
    assert!(
        omitted
            .iter()
            .all(|name| { thinned.iter().all(|text| text.annotation.text != *name) })
    );
    assert!(
        thinned
            .windows(2)
            .all(|pair| { pair[0].bounds().top >= pair[1].bounds().bottom() })
    );
}

#[test]
fn names_never_overlap_when_backgrounds_differ() {
    let mut fig = stack(&NAMES, &[], TraceLabelSide::Right);
    for point in &mut fig.series[1].points {
        point[1] = 1.98;
    }
    let texts = place(&fig, PLOT).texts;
    assert_eq!(texts.len(), 4);
    let mut boxes: Vec<_> = texts.iter().map(TraceText::bounds).collect();
    boxes.sort_by(|a, b| b.top.total_cmp(&a.top));
    assert!(boxes.windows(2).all(|pair| pair[0].top >= pair[1].bottom()));
}

#[test]
fn the_guide_setting_decides_between_names_and_legend() {
    let mut fig = stack(&NAMES, &[], TraceLabelSide::Auto);
    assert!(!crate::renders_legend(&fig));
    assert_eq!(label_layout(&fig, PLOT, 1.0).traces.len(), 4);

    fig.guide_visibility = GuideVisibility::Show;
    assert!(crate::renders_legend(&fig));
    assert!(label_layout(&fig, PLOT, 1.0).traces.is_empty());

    fig.guide_visibility = GuideVisibility::Hide;
    assert!(!crate::renders_legend(&fig));
    assert!(label_layout(&fig, PLOT, 1.0).traces.is_empty());
}

#[test]
fn names_scale_with_the_output_and_reach_every_backend() {
    let fig = stack(&NAMES, &[], TraceLabelSide::Left);
    let points = label_layout(&fig, PLOT, 1.0);
    let doubled = Rect::new(
        PLOT.left * 2.0,
        PLOT.top * 2.0,
        PLOT.width * 2.0,
        PLOT.height * 2.0,
    );
    let pixels = label_layout(&fig, doubled, 2.0);
    for (a, b) in points.traces.iter().zip(&pixels.traces) {
        assert!((a.at.x * 2.0 - b.at.x).abs() < 1e-3);
        assert!((a.at.y * 2.0 - b.at.y).abs() < 1e-3);
    }
    let svg = crate::svg::export(&fig);
    assert!(NAMES.iter().all(|name| svg.contains(name)));
}
