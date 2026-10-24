use super::*;
use plotium_figure::Figure;

fn grouped_axis(min: f64, max: f64) -> Axis {
    let mut axis = Axis::new("x", min, max);
    axis.thousands_separator = true;
    axis
}

#[test]
fn integer_digits_are_grouped_after_the_sign_and_before_the_fraction() {
    assert_eq!(group_integer_digits("0"), "0");
    assert_eq!(group_integer_digits("999"), "999");
    assert_eq!(group_integer_digits("1000"), "1,000");
    assert_eq!(group_integer_digits("-12345.50"), "-12,345.50");
    assert_eq!(group_integer_digits("1234567"), "1,234,567");
}

#[test]
fn separator_is_off_by_default_and_leaves_labels_unchanged() {
    let axis = Axis::new("x", 0.0, 5_000.0);
    assert!(!axis.thousands_separator);
    let ticks = axis_ticks_for(&axis, 5);
    assert!(ticks.labels.iter().any(|label| label == "1000"));
    assert!(ticks.labels.iter().all(|label| !label.contains(',')));
}

#[test]
fn grouped_axis_writes_large_values_in_full_without_a_multiplier() {
    let ticks = axis_ticks_for(&grouped_axis(-500.0, 17_000.0), 5);
    assert_eq!(ticks.scale_exponent, None);
    assert!(ticks.labels.iter().any(|label| label == "15,000"));
    assert!(ticks.labels.iter().any(|label| label == "0"));

    let plain = axis_ticks_for(&Axis::new("x", -500.0, 17_000.0), 5);
    assert_eq!(plain.scale_exponent, Some(4));
}

#[test]
fn grouped_axis_keeps_the_multiplier_for_tiny_values() {
    let ticks = axis_ticks_for(&grouped_axis(0.0, 0.0005), 5);
    assert_eq!(ticks.scale_exponent, Some(-4));
    assert!(ticks.labels.iter().all(|label| !label.contains(',')));
}

#[test]
fn categorical_axes_ignore_the_separator() {
    let mut axis = Axis::categorical("x", vec!["1000".to_owned(), "2000".to_owned()]);
    axis.thousands_separator = true;
    let ticks = axis_ticks_for(&axis, 5);
    assert_eq!(ticks.labels, ["1000", "2000"]);
}

#[test]
fn exported_svg_draws_grouped_labels_and_sizes_margins_for_them() {
    let plain = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 9_000.0));
    let mut grouped = plain.clone();
    grouped.y.thousands_separator = true;

    let svg = crate::svg::export(&grouped);
    assert!(svg.contains(">8,000<"), "grouped y label missing");
    assert!(!crate::svg::export(&plain).contains(">8,000<"));

    let plain_layout = axis_layout(&plain, 300.0, 200.0);
    let grouped_layout = axis_layout(&grouped, 300.0, 200.0);
    assert!(grouped_layout.margins.left > plain_layout.margins.left);
}

#[test]
fn grouped_negative_axis_groups_after_the_sign() {
    let ticks = axis_ticks_for(&grouped_axis(-20_000.0, 0.0), 5);
    assert_eq!(ticks.scale_exponent, None);
    assert!(ticks.labels.iter().any(|label| label == "-15,000"));
    assert!(ticks.labels.iter().any(|label| label == "0"));
    assert!(ticks.labels.iter().all(|label| label != "-0"));
}

#[test]
fn grouped_axis_falls_back_to_the_multiplier_beyond_exact_f64_integers() {
    let exact = axis_ticks_for(&grouped_axis(0.0, 5.0e14), 5);
    assert_eq!(exact.scale_exponent, None);
    assert!(
        exact
            .labels
            .iter()
            .any(|label| label == "100,000,000,000,000")
    );

    let huge = axis_ticks_for(&grouped_axis(0.0, 1.0e24), 5);
    assert_eq!(huge.scale_exponent, Some(24));
    assert!(huge.labels.iter().all(|label| label.len() < 8));
}

#[test]
fn a_category_y_axis_names_every_slot_that_fits() {
    let names: Vec<String> = ["50", "100", "200", "300", "400", "500"]
        .map(String::from)
        .to_vec();
    let mut fig = Figure::new(
        "",
        Axis::new("x", 0.0, 12.0),
        Axis::categorical("", names.clone()),
    );
    fig.y.reversed = true;
    let layout = axis_layout(&fig, 252.0, 170.0);
    assert_eq!(layout.y_ticks.labels, names);
}

#[test]
fn short_panels_keep_endpoint_ticks_before_a_single_middle_tick() {
    let axis = Axis::new("y", 0.0, 10.0);
    let values = |height: f32| adaptive_y_ticks(&axis, 8.0, height).values;
    // Too short for evenly spaced labels, still tall enough for two apart;
    // 10 sits on the top edge, where a joined stack member hides its label.
    assert_eq!(values(20.0), vec![0.0, 5.0]);
    assert_eq!(values(10.0), vec![5.0]);
    assert!(values(7.0).is_empty());
}

#[test]
fn short_reversed_log_axis_keeps_ascending_tick_values() {
    let mut axis = Axis::new("y", 1.0, 1000.0);
    axis.scale = AxisScale::Log10;
    axis.reversed = true;
    let values = adaptive_y_ticks(&axis, 8.0, 20.0).values;
    assert_eq!(values.len(), 2);
    assert!(values[0] < values[1], "{values:?}");
}

#[test]
fn short_panel_ticks_keep_the_axis_label_system() {
    let axis = Axis::new("y", 0.0, 2.0e-5);
    let full = axis_ticks_for(&axis, 2);
    let sparse = adaptive_y_ticks(&axis, 8.0, 20.0);
    assert_eq!(sparse.scale_exponent, full.scale_exponent);
    assert_eq!(sparse.values, full.values[..2]);
    assert_eq!(sparse.labels, full.labels[..2]);
}

#[test]
fn a_coarse_step_with_one_tick_in_range_yields_to_a_finer_pair() {
    // A two-tick target steps by 0.5 here, leaving only 0 inside the range.
    let axis = Axis::new("y", -0.35, 0.42);
    assert_eq!(axis_ticks_for(&axis, 2).values, vec![0.0]);
    for height in [20.0, 30.0, 60.0] {
        assert!(adaptive_y_ticks(&axis, 8.0, height).values.len() >= 2);
    }
}
