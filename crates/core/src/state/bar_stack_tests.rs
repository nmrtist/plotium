//! Stacked bar curves: piles within one share, whiskers and labels on their
//! segments, the stack property and plot.create validation.
use super::*;
use crate::properties::chart_layout;

fn stacked(kind: CurveKind, source: TableCurveSource, stack: &str) -> NewCurve {
    NewCurve {
        stack: Some(stack.into()),
        ..curve(kind, source)
    }
}

#[test]
fn stacked_bars_pile_up_in_one_share_beside_other_stacks() {
    let mut app = fixture();
    let curves = vec![
        stacked(CurveKind::Bar, source(&app, X, Y, None), "s"),
        stacked(CurveKind::Bar, source(&app, X, Y2, Some(ERROR)), "s"),
        stacked(CurveKind::Bar, source(&app, X, Y, None), "t"),
    ];
    create(&mut app, curves);
    let fig = figure(&app);
    assert_eq!(fig.polygons.len(), 12);
    // Two stacks share each unit slot: 0.4 wide bars at x - 0.25 and x + 0.25.
    assert!(close(rect(&fig.polygons[0]), [0.55, 0.95, 0., 2.]));
    assert!(close(rect(&fig.polygons[4]), [0.55, 0.95, 2., 3.]));
    // A negative value hangs below the baseline, not below the positive pile.
    assert!(close(rect(&fig.polygons[2]), [2.55, 2.95, 0., 1.]));
    assert!(close(rect(&fig.polygons[6]), [2.55, 2.95, -1., 0.]));
    assert!(close(rect(&fig.polygons[7]), [3.55, 3.95, 5., 8.]));
    assert!(close(rect(&fig.polygons[8]), [1.05, 1.45, 0., 2.]));
    // Whiskers sit on their own segment's end.
    let centers: Vec<[f64; 2]> = fig.error_bars.iter().map(|e| e.center).collect();
    assert_eq!(
        centers,
        vec![[0.75, 3.], [1.75, 5.], [2.75, -1.], [3.75, 8.]]
    );
    assert!(fig.y.min < -1.2 && fig.y.max > 8.4, "{:?}", fig.y);
}

#[test]
fn stacked_value_labels_sit_inside_their_segments() {
    let mut app = fixture();
    let curves = vec![
        stacked(CurveKind::Bar, source(&app, X, Y, None), "s"),
        stacked(CurveKind::Bar, source(&app, X, Y2, None), "s"),
    ];
    let summary = create(&mut app, curves);
    set(
        &mut app,
        &summary.series_defaults[1].target,
        "series.value_labels",
        json!(true),
    );
    let labels = &figure(&app).annotations;
    assert_eq!(labels.len(), 4);
    assert_eq!(labels[0].text, "1");
    assert_eq!(labels[0].at, [1., 2.5]);
    assert_eq!(labels[2].at, [3., -0.5]);
    assert!(labels.iter().all(|l| l.anchor.is_point()));
}

#[test]
fn bar_stack_is_a_series_property_offered_only_for_bars() {
    let mut app = fixture();
    let curves = vec![
        curve(CurveKind::Bar, source(&app, X, Y, None)),
        curve(CurveKind::Bar, source(&app, X, Y2, None)),
        curve(CurveKind::Line, source(&app, X, Y2, None)),
    ];
    let summary = create(&mut app, curves);
    let target = |i: usize| summary.series_defaults[i].target.clone();
    let read = |app: &PlotiumApp, i| {
        app.resolve_property(&PropertyAddress::new(target(i), chart_layout::BAR_STACK))
            .unwrap()
    };
    assert_eq!(
        read(&app, 0).value,
        AggregateValue::Uniform(PropertyValue::Text(String::new()))
    );
    assert!(matches!(
        read(&app, 2).availability,
        Availability::Disabled(_)
    ));
    for i in [0, 1] {
        set(&mut app, &target(i), "series.bar.stack", json!(" total "));
    }
    assert_eq!(
        read(&app, 1).value,
        AggregateValue::Uniform(PropertyValue::Text("total".into()))
    );
    // One stack alone in its slot takes the whole slot.
    assert!(close(rect(&figure(&app).polygons[4]), [0.6, 1.4, 2., 3.]));
    set(&mut app, &target(1), "series.bar.stack", json!(""));
    assert_eq!(read(&app, 1).modified, Some(false));
    assert!(close(rect(&figure(&app).polygons[4]), [1.05, 1.45, 0., 1.]));
}

#[test]
fn plot_create_takes_a_stack_only_for_bars() {
    let app = fixture();
    let error = plan(
        &app,
        vec![stacked(CurveKind::Line, source(&app, X, Y, None), "s")],
    )
    .unwrap_err();
    assert!(error.contains("Only bar curves stack"), "{error}");
    let error = plan(
        &app,
        vec![stacked(CurveKind::Bar, source(&app, X, Y, None), "  ")],
    )
    .unwrap_err();
    assert!(error.contains("nonblank"), "{error}");
}

#[test]
fn stack_names_survive_a_project_round_trip() {
    let mut app = fixture();
    let curves = vec![
        stacked(CurveKind::Bar, source(&app, X, Y, None), "s"),
        stacked(CurveKind::Bar, source(&app, X, Y2, None), "s"),
    ];
    create(&mut app, curves);
    let path = std::env::temp_dir().join(format!(
        "plotium-bar-stack-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let rects = |app: &PlotiumApp| figure(app).polygons.iter().map(rect).collect::<Vec<_>>();
    assert_eq!(rects(&loaded), rects(&app));
}

#[test]
fn value_axis_follows_lifted_segments_not_raw_values() {
    let mut app = fixture();
    // Top whiskers reach below zero before lifting, but not after.
    let curves = vec![
        stacked(CurveKind::Bar, source(&app, X, Y, None), "s"),
        stacked(CurveKind::Bar, source(&app, X, ERROR, Some(Y)), "s"),
    ];
    let summary = create(&mut app, curves);
    assert_eq!(figure(&app).y.min, 0.0);
    let request = ToolRequest {
        tool_id: "properties.set".into(),
        tool_version: 1,
        parameters: json!({"key": "object.chart.orientation", "value": "horizontal"}),
        targets: TargetSelector::Explicit {
            ids: vec![summary.plot.resource.id.clone()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(&app, request).unwrap();
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    let fig = figure(&app);
    assert_eq!(fig.x.min, 0.0);
    assert!(fig.x.max > 10.4, "{:?}", fig.x);
}
