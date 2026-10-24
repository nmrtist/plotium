use super::*;
use crate::{Annotation, Axis, Color, ErrorBar, Series, TextAnchor};

fn signed_stack() -> Figure {
    let mut figure = Figure::new("stack", Axis::new("x", 0., 2.), Axis::new("y", -3., 15.));
    figure
        .series
        .push(Series::line("first", vec![[0., 0.], [1., 2.], [2., -1.]]));
    let mut second = Series::line("second", vec![[0., 10.], [1., 11.], [2., 9.5]]);
    second.display_baseline = 10.;
    figure.series.push(second);
    let mut error = ErrorBar::symmetric([1., 11.], 0.5);
    error.display_baseline = 10.;
    figure.error_bars.push(error);
    figure
}

#[test]
fn gain_keeps_each_baseline_and_signed_relative_heights() {
    let mut figure = signed_stack();
    figure.set_display_gain(2.).unwrap();
    assert_eq!(figure.series[0].points, [[0., 0.], [1., 4.], [2., -2.]]);
    assert_eq!(figure.series[1].points, [[0., 10.], [1., 12.], [2., 9.]]);
    assert_eq!(figure.error_bars[0].center, [1., 12.]);
    assert_eq!(figure.error_bars[0].negative, 1.);
    assert_eq!((figure.y.min, figure.y.max), (-3., 15.));
    figure.set_display_gain(2.).unwrap();
    assert_eq!(figure.series[1].points[1][1], 12.);
    figure.set_display_gain(1.).unwrap();
    assert_eq!(figure.series[1].points, signed_stack().series[1].points);
}

#[test]
fn materialized_gain_roundtrip_does_not_apply_twice() {
    let mut figure = signed_stack();
    figure.set_display_gain(3.).unwrap();
    let mut restored: Figure =
        serde_json::from_str(&serde_json::to_string(&figure).unwrap()).unwrap();
    restored.set_display_gain(3.).unwrap();
    assert_eq!(restored.series[1].points, figure.series[1].points);
    restored.set_display_gain(1.).unwrap();
    assert_eq!(restored.series[1].points, signed_stack().series[1].points);
}

#[test]
fn invalid_or_overflowing_gain_leaves_geometry_unchanged() {
    let mut figure = signed_stack();
    for gain in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(figure.set_display_gain(gain).is_err());
        assert_eq!(figure.series[1].points, signed_stack().series[1].points);
    }
    figure.series[0].points[1][1] = f64::MAX;
    assert!(figure.set_display_gain(2.).is_err());
    assert_eq!(figure.display_gain, 1.);
    assert_eq!(figure.series[0].points[1][1], f64::MAX);
}

#[test]
fn repeated_wheel_steps_and_reset_do_not_accumulate_roundoff() {
    let mut stepped = signed_stack();
    stepped.series[1].points[1][1] = 10.123456789;
    let original = stepped.clone();
    let mut direct = original.clone();
    for gain in [1.13, 2.63, 0.00001, 3.27] {
        stepped.set_display_gain(gain).unwrap();
    }
    direct.set_display_gain(3.27).unwrap();
    assert_eq!(stepped.series[1].points, direct.series[1].points);
    stepped.set_display_gain(1.).unwrap();
    assert_eq!(stepped.series[1].points, original.series[1].points);
}

#[test]
fn stacked_value_labels_follow_their_trace_through_gain_and_snapshot_restore() {
    let mut figure = signed_stack();
    for y in [10., 11., 9.5] {
        figure.annotations.push(Annotation {
            text: (y - 10.).to_string(),
            at: [1., y],
            display_baseline: 10.,
            color: Color::BLACK,
            size: 8.,
            anchor: TextAnchor::Above,
            offset_pt: None,
        });
    }
    figure.set_display_gain(2.).unwrap();
    for (label, point) in figure.annotations.iter().zip(&figure.series[1].points) {
        assert_eq!(label.at[1], point[1]);
    }
    assert_eq!(figure.annotations[1].text, "1");
    let mut restored: Figure =
        serde_json::from_str(&serde_json::to_string(&figure).unwrap()).unwrap();
    for gain in [3., 0.5, 1.] {
        restored.set_display_gain(gain).unwrap();
        for (label, point) in restored.annotations.iter().zip(&restored.series[1].points) {
            assert_eq!(label.at[1], point[1]);
        }
    }
}

#[test]
fn invalid_or_overflowing_label_gain_is_rejected_before_mutating_geometry() {
    let mut figure = signed_stack();
    figure.annotations.push(Annotation {
        text: "peak".into(),
        at: [1., 1.],
        display_baseline: f64::MAX,
        color: Color::BLACK,
        size: 8.,
        anchor: TextAnchor::Above,
        offset_pt: None,
    });
    assert!(figure.set_display_gain(3.).is_err());
    figure.annotations[0].display_baseline = f64::NAN;
    assert!(figure.set_display_gain(2.).is_err());
    assert_eq!(figure.display_gain, 1.);
    assert_eq!(figure.series[1].points, signed_stack().series[1].points);
    assert_eq!(figure.annotations[0].at, [1., 1.]);
}
