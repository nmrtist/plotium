use super::axis_scale_tests::{figure, line_app, manual, write};
use super::*;
use crate::state::PlotiumApp;

fn resolve(
    app: &PlotiumApp,
    target: &crate::automation::TargetRef,
    id: PropertyId,
) -> ResolvedProperty {
    app.resolve_property(&PropertyAddress::new(target.clone(), id))
        .expect("property resolves")
}

#[test]
fn x_break_hides_its_values_and_undoes() {
    let (mut app, target, id) = line_app();
    let fraction = resolve(&app, &target, plot_display::X_BREAK_FRACTION);
    assert!(matches!(fraction.availability, Availability::Disabled(_)));

    write(&mut app, &target, plot_display::X_BREAK, manual(30.0, 70.0));
    let x = &figure(&app, id).x;
    assert!(x.hides(50.0) && !x.hides(20.0) && !x.hides(80.0));
    let resolved = resolve(&app, &target, plot_display::X_BREAK);
    assert_eq!(resolved.value, AggregateValue::Uniform(manual(30.0, 70.0)));
    assert_eq!(resolved.modified, Some(true));

    let fraction = resolve(&app, &target, plot_display::X_BREAK_FRACTION);
    assert_eq!(fraction.availability, Availability::Editable);
    let Some(PropertyValue::Float(natural)) = fraction.default_value else {
        panic!("fraction has a numeric default");
    };
    assert!(natural > 0.3 && natural < 0.6, "{natural}");

    write(
        &mut app,
        &target,
        plot_display::X_BREAK_FRACTION,
        PropertyValue::Float(0.7),
    );
    let brk = figure(&app, id).x.axis_break.expect("break kept");
    assert_eq!(brk.lower_fraction, Some(0.7));
    // Moving the break keeps the chosen share.
    write(&mut app, &target, plot_display::X_BREAK, manual(40.0, 60.0));
    let brk = figure(&app, id).x.axis_break.expect("break kept");
    assert_eq!(
        (brk.from, brk.to, brk.lower_fraction),
        (40.0, 60.0, Some(0.7))
    );

    app.undo();
    app.undo();
    app.undo();
    assert!(figure(&app, id).x.axis_break.is_none());

    app.redo();
    write(
        &mut app,
        &target,
        plot_display::X_BREAK,
        PropertyValue::AxisRange(AxisRangeValue::Auto),
    );
    assert!(figure(&app, id).x.axis_break.is_none());
}

#[test]
fn a_break_outside_the_axis_or_on_a_log_axis_is_refused() {
    let (mut app, target, _) = line_app();
    assert!(
        app.plan_property_write(
            plot_display::X_BREAK,
            std::slice::from_ref(&target),
            &manual(50.0, 500.0),
        )
        .is_err()
    );

    write(
        &mut app,
        &target,
        axis::X_SCALE,
        PropertyValue::Enum(axis::LOG),
    );
    let resolved = resolve(&app, &target, plot_display::X_BREAK);
    assert!(matches!(resolved.availability, Availability::Disabled(_)));
    let refused = app
        .plan_property_write(
            plot_display::X_BREAK,
            std::slice::from_ref(&target),
            &manual(30.0, 70.0),
        )
        .map_or(true, |commit| !commit.skipped.is_empty());
    assert!(refused);
}
