use super::*;
use crate::svg::export_document;
use crate::{Document, DocumentItem, DocumentObject};
use plotium_figure::{Axis, Color, Series};

fn figure(y_label: &str, y_max: f64, x_label: &str) -> Figure {
    Figure::new(
        "",
        Axis::new(x_label, 10.0, 80.0),
        Axis::new(y_label, 0.0, y_max),
    )
    .with_series(Series::line("s", vec![[10.0, 0.0], [80.0, y_max]]))
}

fn placement(joined_above: bool, joined_below: bool) -> StackPlacement {
    StackPlacement {
        left: 40.0,
        right: 12.0,
        joined_above,
        joined_below,
    }
}

#[test]
fn placement_forces_shared_margins_and_closes_inner_boundaries() {
    let fig = figure("Intensity", 1000.0, "2θ");
    let natural = axis_layout(&fig, 240.0, 100.0);
    assert!(natural.margins.top > 0.0 && natural.margins.bottom > 0.0);

    let middle = axis_layout_placed(&fig, 240.0, 100.0, Some(&placement(true, true)));
    assert_eq!(middle.margins.left, 40.0);
    assert_eq!(middle.margins.right, 12.0);
    assert_eq!(middle.margins.top, 0.0);
    assert_eq!(middle.margins.bottom, 0.0);

    let bottom = axis_layout_placed(&fig, 240.0, 100.0, Some(&placement(true, false)));
    assert_eq!(bottom.margins.top, 0.0);
    assert_eq!(bottom.margins.bottom, natural.margins.bottom);
    assert_eq!(axis_layout_placed(&fig, 240.0, 100.0, None), natural);
}

#[test]
fn members_with_different_y_labels_share_one_x_tick_set() {
    let narrow = figure("y", 1.0, "x");
    let wide = figure("Counts per second", 125000.0, "x");
    let top = axis_layout_placed(&narrow, 240.0, 90.0, Some(&placement(false, true)));
    let bottom = axis_layout_placed(&wide, 240.0, 90.0, Some(&placement(true, false)));
    assert_eq!(top.x_ticks, bottom.x_ticks);
}

#[test]
fn hidden_axes_ignore_placement() {
    let mut fig = figure("y", 1.0, "x");
    fig.axis_frame = AxisFrame::Hidden;
    let p = placement(true, true);
    assert_eq!(
        axis_layout_placed(&fig, 200.0, 100.0, Some(&p)),
        axis_layout(&fig, 200.0, 100.0)
    );
    assert!(XFurniture::of(&fig, Some(&p)).show_title);
}

#[test]
fn inner_boundary_furniture_points_inward_and_hides_x_text() {
    let fig = figure("y", 1.0, "x");
    let inner = XFurniture::of(&fig, Some(&placement(false, true)));
    assert!(!inner.show_tick_labels && !inner.show_label);
    assert_eq!(inner.tick_reach, -fig.typography.tick_length_pt);
    assert!(inner.minor_tick_reach < 0.0);

    let bottom = XFurniture::of(&fig, Some(&placement(true, false)));
    assert!(bottom.show_tick_labels && bottom.show_label && !bottom.show_title);
    assert_eq!(bottom.tick_reach, fig.typography.tick_reach());
    assert!(bottom.background_inset > 0.0);
}

#[test]
fn svg_stack_draws_x_text_only_on_the_bottom_member() {
    let upper = figure("Upper Y", 1.0, "Upper X");
    let lower = figure("Lower Y", 1.0, "Lower X");
    let object = |id: &str, figure, top: f32, stack| {
        DocumentItem::Plot(DocumentObject {
            id: id.into(),
            frame: Rect::new(0.0, top, 240.0, 90.0),
            figure,
            geometry_generation: None,
            visible: true,
            title: None,
            y_title_clearance: 0.0,
            stack: Some(stack),
        })
    };
    let document = Document {
        width: 240.0,
        height: 180.0,
        background: Color::rgb(255, 255, 255),
        items: vec![
            object("upper", &upper, 0.0, placement(false, true)),
            object("lower", &lower, 90.0, placement(true, false)),
        ],
    };
    let svg = export_document(&document);
    assert!(!svg.contains("Upper X"));
    assert!(svg.contains("Lower X"));
    assert!(svg.contains("Upper Y") && svg.contains("Lower Y"));

    let upper_group = svg
        .split(r#"<g id="lower""#)
        .next()
        .expect("upper group precedes lower");
    let inward = format!("v{}\"", -upper.typography.tick_length_pt);
    assert!(upper_group.contains(&inward), "upper x ticks point inward");
    let lower_group = svg.split(r#"<g id="lower""#).nth(1).expect("lower group");
    let outward = format!("v{}\"", upper.typography.tick_length_pt);
    assert!(
        lower_group.contains(&outward),
        "bottom x ticks keep direction"
    );
}

#[test]
fn joined_top_edge_moves_the_y_multiplier_inside_and_skips_its_edge_label() {
    let fig = figure("Counts", 125_000.0, "x");
    let below = XFurniture::of(&fig, Some(&placement(true, false)));
    assert!(below.y_multiplier_inside);
    assert!(!below.shows_y_label_at(0.0) && below.shows_y_label_at(fig.typography.tick_pt));
    let top = XFurniture::of(&fig, Some(&placement(false, true)));
    assert!(!top.y_multiplier_inside && top.shows_y_label_at(0.0));

    let document = Document {
        width: 240.0,
        height: 90.0,
        background: Color::rgb(255, 255, 255),
        items: vec![DocumentItem::Plot(DocumentObject {
            id: "lower".into(),
            frame: Rect::new(0.0, 0.0, 240.0, 90.0),
            figure: &fig,
            geometry_generation: None,
            visible: true,
            title: None,
            y_title_clearance: 0.0,
            stack: Some(placement(true, false)),
        })],
    };
    let svg = export_document(&document);
    let inside = format!(
        r#"y="{:.2}" text-anchor="start""#,
        crate::TICK_LABEL_PAD + fig.typography.tick_pt
    );
    assert!(svg.contains(&inside), "multiplier sits inside the plot top");
}
