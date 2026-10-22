use super::*;
use crate::automation::*;
use crate::properties::{self, AxisRangeValue, PropertyAddress, PropertyValue};
use plotium_figure::{Color, LineStyle, Marker, SeriesEncoding};
use serde_json::json;

fn fixture() -> PlotiumApp {
    let mut app = super::curve_tests::fixture(false);
    app.doc.canvases.clear();
    app.session.active_canvas = None;
    let dataset = &app.doc.datasets[0];
    let t = dataset.as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let source = TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[0].id,
        y_column_id: c[1].id,
        y_error_column_id: Some(c[2].id),
    };
    for name in ["Edited", "Untouched"] {
        let p = PlotCreateParams {
            parent: document_root_target(),
            name: name.into(),
            canvas: None,
            panel_name: None,
            panel_frame_pt: None,
            series: vec![
                NewCurve {
                    y_axis: Default::default(),
                    kind: CurveKind::Line,
                    source: source.clone(),
                },
                NewCurve {
                    y_axis: Default::default(),
                    kind: CurveKind::Scatter,
                    source: source.clone(),
                },
            ],
        };
        let plan = app
            .plan_plot_creation(DocumentRevision(app.doc.automation_revision), &p)
            .unwrap();
        app.commit_curve_bindings(plan).unwrap();
    }
    app.clear_history();
    app
}
fn plot(app: &PlotiumApp, page: usize) -> &PlotObject {
    app.doc.canvases[page].objects[0].plot().unwrap()
}
fn target(app: &PlotiumApp, page: usize, series: usize) -> TargetRef {
    let o = &app.doc.canvases[page].objects[0];
    app.series_target(page, o.id, o.plot().unwrap().binding.series[series].id)
        .unwrap()
}
fn object(app: &PlotiumApp, page: usize) -> TargetRef {
    app.object_target(page, app.doc.canvases[page].objects[0].id)
        .unwrap()
}
fn request(
    app: &PlotiumApp,
    key: &str,
    value: serde_json::Value,
    targets: Vec<TargetRef>,
) -> ToolRequest {
    ToolRequest {
        tool_id: "properties.set".into(),
        tool_version: 1,
        parameters: json!({"key":key,"value":value}),
        targets: if targets.iter().all(|t| t.component.is_none()) {
            TargetSelector::Explicit {
                ids: targets.into_iter().map(|t| t.resource.id).collect(),
            }
        } else {
            TargetSelector::Components { targets }
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}
fn write(
    app: &mut PlotiumApp,
    key: properties::PropertyId,
    value: PropertyValue,
    targets: &[TargetRef],
) {
    let p = app.plan_property_write(key, targets, &value).unwrap();
    app.commit_property(p);
}
#[test]
fn r6_exact_id_preserves_same_name_neighbor_binding_ranges_and_geometry() {
    let mut app = fixture();
    let a = target(&app, 0, 0);
    let b = target(&app, 0, 1);
    assert_eq!(
        app.series_label(&plot(&app, 0).binding.series[0]),
        app.series_label(&plot(&app, 0).binding.series[1])
    );
    let binding = plot(&app, 0).binding.clone();
    let other = plot(&app, 1).figure().clone();
    let frames = app
        .doc
        .canvases
        .iter()
        .map(|c| (c.objects[0].frame, c.panels[0].frame))
        .collect::<Vec<_>>();
    let viewport = plot(&app, 0).viewport.clone();
    let req = request(&app, "series.line.color", json!("#0072B2"), vec![a.clone()]);
    let p = plan_tool(&app, req).unwrap();
    execute_tool(&mut app, p, ExecutionAuthority::ReversibleModify).unwrap();
    let updated = plot(&app, 0);
    assert_eq!(updated.binding.series[1], binding.series[1]);
    let SeriesEncoding::Line(line) = &updated.binding.series[0].encoding else {
        panic!()
    };
    assert_eq!(line.color.resolve(), Color::rgb(0, 114, 178));
    assert_eq!(line.width.get(), 0.5);
    assert_eq!(updated.binding.series[0].source, binding.series[0].source);
    assert_eq!(updated.binding.coordinates, binding.coordinates);
    assert_eq!(updated.viewport, viewport);
    assert_eq!(
        serde_json::to_value(plot(&app, 1).figure()).unwrap(),
        serde_json::to_value(other).unwrap()
    );
    assert_eq!(
        frames,
        app.doc
            .canvases
            .iter()
            .map(|c| (c.objects[0].frame, c.panels[0].frame))
            .collect::<Vec<_>>()
    );
    app.undo();
    assert_eq!(plot(&app, 0).binding, binding);
    app.redo();
    assert_eq!(target(&app, 0, 0), a);
    assert_eq!(target(&app, 0, 1), b);
}
#[test]
fn r6_display_units_keep_coordinate_ranges_data_titles_and_local_fonts() {
    use properties::plot_display::*;
    let mut app = fixture();
    let t = object(&app, 0);
    let binding = plot(&app, 0).binding.clone();
    let source = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .clone();
    write(
        &mut app,
        Y_RANGE,
        PropertyValue::AxisRange(AxisRangeValue::Manual {
            min: 0.,
            max: 10.,
            unit: Some("mA".into()),
        }),
        std::slice::from_ref(&t),
    );
    write(
        &mut app,
        properties::axis::X_LABEL,
        PropertyValue::Text("Time authored".into()),
        std::slice::from_ref(&t),
    );
    let before = plot(&app, 0).viewport.clone();
    write(
        &mut app,
        Y_UNIT,
        PropertyValue::Text("A".into()),
        std::slice::from_ref(&t),
    );
    write(
        &mut app,
        X_UNIT,
        PropertyValue::Text("ms".into()),
        std::slice::from_ref(&t),
    );
    assert_eq!(plot(&app, 0).viewport, before);
    assert_eq!(plot(&app, 0).binding, binding);
    assert_eq!(
        plot(&app, 0).axis_overrides.y_range,
        Some(AxisRange::new(0., 10.))
    );
    assert_eq!(plot(&app, 0).figure().x.label, "Time authored");
    let automatic = app
        .resolve_property(&PropertyAddress::new(t.clone(), properties::axis::Y_LABEL))
        .unwrap();
    assert_eq!(
        automatic.default_value,
        Some(PropertyValue::Text(plot(&app, 0).figure().y.label.clone()))
    );
    assert!(!automatic.is_modified());
    assert!(plot(&app, 0).figure().y.label.ends_with(" (A)"));
    assert_eq!(
        plot(&app, 0)
            .figure()
            .y
            .display
            .as_ref()
            .unwrap()
            .to_display(10.),
        0.01
    );
    assert_eq!(
        app.doc.datasets[0]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .id,
        source.id
    );
    for (key, size) in [(TICK_PT, 11.), (LABEL_PT, 12.), (LEGEND_PT, 10.)] {
        write(
            &mut app,
            key,
            PropertyValue::Float(size),
            std::slice::from_ref(&t),
        );
    }
    assert_eq!(plot(&app, 0).figure().typography.tick_pt, 11.);
    assert_eq!(plot(&app, 1).figure().typography.tick_pt, 7.);
    let doc = app.document_target();
    write(
        &mut app,
        properties::typography::TICK_PT,
        PropertyValue::Float(9.),
        &[doc],
    );
    assert_eq!(plot(&app, 0).figure().typography.tick_pt, 11.);
    assert_eq!(plot(&app, 1).figure().typography.tick_pt, 9.);
    let resolved = app
        .resolve_property(&PropertyAddress::new(t.clone(), Y_RANGE))
        .unwrap();
    assert_eq!(
        resolved.value,
        properties::AggregateValue::Uniform(PropertyValue::AxisRange(AxisRangeValue::Manual {
            min: 0.,
            max: 0.01,
            unit: Some("A".into())
        }))
    );
    let x_range = plot(&app, 0).axis_overrides.x_range;
    let title = plot(&app, 0).axis_overrides.x_label.clone();
    write(
        &mut app,
        Y_RANGE,
        PropertyValue::AxisRange(AxisRangeValue::Auto),
        &[t],
    );
    assert!(plot(&app, 0).axis_overrides.y_range.is_none());
    assert_eq!(plot(&app, 0).axis_overrides.x_range, x_range);
    assert_eq!(plot(&app, 0).axis_overrides.x_label, title);
    assert!(plot(&app, 0).axis_overrides.y_display_unit.is_some());
}
#[test]
fn r6_preflight_rejects_invalid_values_and_missing_identity_but_reports_skips() {
    let mut app = fixture();
    let t = object(&app, 0);
    let a = target(&app, 0, 0);
    let b = target(&app, 0, 1);
    let revision = app.doc.automation_revision;
    for (key, value, targets) in [
        ("series.line.color", json!("red"), vec![a.clone()]),
        ("series.line.marker_size", json!(0.), vec![a.clone()]),
        (
            "object.axes.y_range",
            json!({"mode":"manual","min":2.,"max":1.,"unit":"mA"}),
            vec![t.clone()],
        ),
        (
            "object.axes.y_range",
            json!({"mode":"manual","min":1.,"max":1.,"unit":"mA"}),
            vec![t.clone()],
        ),
        ("object.axes.y_display_unit", json!("s"), vec![t]),
    ] {
        assert!(
            matches!(
                plan_tool(&app, request(&app, key, value, targets)),
                Err(AutomationError::InvalidParameters { .. })
            ),
            "{key}"
        );
    }
    assert!(
        app.plan_property_write(
            properties::line::STROKE_WIDTH,
            std::slice::from_ref(&a),
            &PropertyValue::Float(f64::NAN)
        )
        .is_err()
    );
    let mut missing = a.clone();
    missing.component = Some(ComponentRef::Series(SeriesId::new(9999)));
    assert!(matches!(
        plan_tool(
            &app,
            request(
                &app,
                "series.line.color",
                json!("#123456"),
                vec![a.clone(), missing]
            )
        ),
        Err(AutomationError::InvalidSelector(_))
    ));
    assert_eq!(app.doc.automation_revision, revision);
    assert!(!app.can_undo());
    let p = plan_tool(
        &app,
        request(&app, "series.line.style", json!("dashed"), vec![a, b]),
    )
    .unwrap();
    let result = execute_tool(&mut app, p, ExecutionAuthority::ReversibleModify).unwrap();
    assert_eq!(
        result
            .targets
            .iter()
            .filter(|t| t.outcome == TargetOutcome::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        result
            .targets
            .iter()
            .filter(|t| t.outcome == TargetOutcome::Skipped)
            .count(),
        1
    );
    let SeriesEncoding::Line(line) = &plot(&app, 0).binding.series[0].encoding else {
        panic!()
    };
    assert_eq!(line.appearance.line_style, LineStyle::Dashed);
    app.undo();
    assert!(!app.can_undo());
}
#[test]
fn r6_shared_batch_one_undo_and_v1_roundtrip_preserve_native_display() {
    let mut app = fixture();
    let a = target(&app, 0, 0);
    let b = target(&app, 0, 1);
    let t = object(&app, 0);
    let binding = plot(&app, 0).binding.clone();
    write(
        &mut app,
        properties::line_display::LABEL,
        PropertyValue::Text("Same label".into()),
        &[a.clone(), b.clone()],
    );
    app.undo();
    assert_eq!(plot(&app, 0).binding, binding);
    assert!(!app.can_undo());
    app.redo();
    write(
        &mut app,
        properties::line_display::MARKER,
        PropertyValue::Enum("diamond"),
        std::slice::from_ref(&a),
    );
    write(
        &mut app,
        properties::line_display::MARKER_SIZE,
        PropertyValue::Float(5.),
        std::slice::from_ref(&a),
    );
    write(
        &mut app,
        properties::plot_display::Y_UNIT,
        PropertyValue::Text("A".into()),
        std::slice::from_ref(&t),
    );
    write(
        &mut app,
        properties::plot_display::LABEL_PT,
        PropertyValue::Float(11.),
        &[t],
    );
    let SeriesEncoding::Line(line) = &plot(&app, 0).binding.series[0].encoding else {
        panic!()
    };
    assert_eq!(line.appearance.marker, Some(Marker::Diamond));
    assert_eq!(line.appearance.marker_radius(line.width.get()), 2.5);
    for snapshot in [false, true] {
        let path = std::env::temp_dir().join(format!(
            "plotium-r6-{}-{snapshot}.plotium",
            uuid::Uuid::new_v4()
        ));
        crate::project::save_project(&app, &path, snapshot).unwrap();
        let loaded = crate::project::load_project(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(plot(&loaded, 0).binding, plot(&app, 0).binding);
        assert_eq!(
            plot(&loaded, 0).axis_overrides,
            plot(&app, 0).axis_overrides
        );
        assert_eq!(target(&loaded, 0, 0), a);
        assert_eq!(target(&loaded, 0, 1), b);
        assert_eq!(
            plot(&loaded, 0).figure().y.display,
            plot(&app, 0).figure().y.display
        );
        assert_eq!(
            plot(&loaded, 0).figure().typography,
            plot(&app, 0).figure().typography
        );
    }
}

#[test]
fn r6_affine_and_domain_unit_range_inputs_retain_native_meaning() {
    use plotium_data::{ColumnChunk, ColumnSchema, LogicalType, UnitRegistry};
    for unit in [
        UnitRegistry::plotium_v1().resolve("K").unwrap(),
        UnitRegistry::plotium_v1()
            .registered_domain_unit("counts", "plotium.test.r6.counts")
            .unwrap(),
    ] {
        let mut app = PlotiumApp::new();
        let mut y = ColumnSchema::new("Signal", LogicalType::Float64);
        y.unit = Some(unit.clone());
        let x = ColumnSchema::new("x", LogicalType::Float64);
        let xi = x.id;
        let yi = y.id;
        let t = TableDataset::from_materialized(
            vec![
                (x, ColumnChunk::optional_f64([Some(0.), Some(1.)])),
                (y, ColumnChunk::optional_f64([Some(280.), Some(300.)])),
            ],
            vec![],
            None,
            vec![],
            "plotium.test.r6.units",
        )
        .unwrap();
        let source = TableCurveSource {
            dataset_id: t.resource_id,
            field_id: FieldId::new(0),
            table_id: t.typed_state.envelope.revision.table_id,
            table_revision: t.typed_state.envelope.revision.id,
            x_column_id: xi,
            y_column_id: yi,
            y_error_column_id: None,
        };
        app.doc.datasets.push(Dataset::Table(Box::new(t)));
        let p = PlotCreateParams {
            parent: document_root_target(),
            name: "Units".into(),
            canvas: None,
            panel_name: None,
            panel_frame_pt: None,
            series: vec![NewCurve {
                y_axis: Default::default(),
                kind: CurveKind::Line,
                source,
            }],
        };
        let plan = app
            .plan_plot_creation(DocumentRevision(app.doc.automation_revision), &p)
            .unwrap();
        app.commit_curve_bindings(plan).unwrap();
        let t = object(&app, 0);
        let (code, min, max, expected) = if unit.display_unit == "K" {
            ("Cel", 0., 100., AxisRange::new(273.15, 373.15))
        } else {
            ("counts", 270., 310., AxisRange::new(270., 310.))
        };
        write(
            &mut app,
            properties::plot_display::Y_RANGE,
            PropertyValue::AxisRange(AxisRangeValue::Manual {
                min,
                max,
                unit: Some(code.into()),
            }),
            std::slice::from_ref(&t),
        );
        assert_eq!(plot(&app, 0).axis_overrides.y_range, Some(expected));
        write(
            &mut app,
            properties::plot_display::Y_UNIT,
            PropertyValue::Text(code.into()),
            std::slice::from_ref(&t),
        );
        assert_eq!(plot(&app, 0).axis_overrides.y_range, Some(expected));
        if code == "Cel" {
            // Finite coordinate endpoints can collapse after an affine offset.
            assert!(
                app.plan_property_write(
                    properties::plot_display::Y_RANGE,
                    &[t],
                    &PropertyValue::AxisRange(AxisRangeValue::Manual {
                        min: 0.,
                        max: 1e-20,
                        unit: Some("K".into()),
                    }),
                )
                .is_err()
            );
            assert_eq!(plot(&app, 0).axis_overrides.y_range, Some(expected));
        }
    }
}

#[test]
fn r6_inapplicable_marker_and_semantic_color_are_reported_before_commit() {
    let mut app = fixture();
    let target = target(&app, 0, 0);
    let object = app.doc.canvases[0].objects[0].id;
    let mut binding = plot(&app, 0).binding.clone();
    let SeriesEncoding::Line(line) = &mut binding.series[0].encoding else {
        panic!()
    };
    line.kind = plotium_figure::SeriesKind::Stick;
    // This fixture exercises the provider gate directly; explicit table binding
    // intentionally has no stick construction operation.
    app.doc.canvases[0]
        .object_mut(object)
        .unwrap()
        .plot_mut()
        .unwrap()
        .binding = binding;
    let p = app
        .plan_property_write(
            properties::line_display::MARKER,
            std::slice::from_ref(&target),
            &PropertyValue::Enum("circle"),
        )
        .unwrap();
    assert!(p.applied.is_empty());
    assert_eq!(p.skipped[0].reason, properties::SkipReason::NotApplicable);
    let mut figure = plot(&app, 0).figure().clone();
    figure.series_colors_are_semantic = true;
    app.doc.canvases[0]
        .object_mut(object)
        .unwrap()
        .plot_mut()
        .unwrap()
        .adopt_rebuilt_figure(figure);
    let p = plan_tool(
        &app,
        request(&app, "series.line.color", json!("#0072B2"), vec![target]),
    )
    .unwrap();
    assert_eq!(p.targets[0].status, TargetCompatibility::Skipped);
    assert!(!app.can_undo());
}

#[test]
fn r6_rebind_reorder_remove_keep_display_units_manual_ranges_and_authored_style() {
    let mut app = fixture();
    let owner = object(&app, 0);
    let first = target(&app, 0, 0);
    let second = target(&app, 0, 1);
    write(
        &mut app,
        properties::plot_display::Y_RANGE,
        PropertyValue::AxisRange(AxisRangeValue::Manual {
            min: 0.,
            max: 10.,
            unit: Some("mA".into()),
        }),
        std::slice::from_ref(&owner),
    );
    write(
        &mut app,
        properties::plot_display::X_UNIT,
        PropertyValue::Text("ms".into()),
        std::slice::from_ref(&owner),
    );
    write(
        &mut app,
        properties::plot_display::Y_UNIT,
        PropertyValue::Text("A".into()),
        std::slice::from_ref(&owner),
    );
    write(
        &mut app,
        properties::plot_display::TICK_PT,
        PropertyValue::Float(9.),
        std::slice::from_ref(&owner),
    );
    write(
        &mut app,
        properties::axis::X_LABEL,
        PropertyValue::Text("Authored time".into()),
        std::slice::from_ref(&owner),
    );
    write(
        &mut app,
        properties::line_display::MARKER,
        PropertyValue::Enum("diamond"),
        std::slice::from_ref(&first),
    );
    let before = plot(&app, 0).binding.clone();
    let overrides = plot(&app, 0).axis_overrides.clone();
    // Individually finite bounds must still be refused if display conversion overflows.
    assert!(
        app.plan_property_write(
            properties::plot_display::X_RANGE,
            std::slice::from_ref(&owner),
            &PropertyValue::AxisRange(AxisRangeValue::Manual {
                min: 1e307,
                max: 1.1e307,
                unit: Some("s".into())
            })
        )
        .is_err()
    );
    let dataset = &app.doc.datasets[0];
    let table = dataset.as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let source = TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[7].id,
        y_column_id: c[6].id,
        y_error_column_id: Some(c[8].id),
    };
    let p = app
        .plan_curve_bindings(
            DocumentRevision(app.doc.automation_revision),
            &[CurveBindingEdit {
                target: first.clone(),
                source,
            }],
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    assert_eq!(
        plot(&app, 0).binding.series[0].encoding,
        before.series[0].encoding
    );
    assert_eq!(plot(&app, 0).binding.series[1], before.series[1]);
    assert_eq!(plot(&app, 0).binding.coordinates, before.coordinates);
    assert_eq!(plot(&app, 0).axis_overrides, overrides);
    assert_eq!(plot(&app, 0).figure().typography.tick_pt, 9.);
    assert_eq!(plot(&app, 0).figure().x.label, "Authored time");
    let mut order = plot(&app, 0).binding.clone();
    order.series.remove(0);
    let canvas = app.doc.canvases[0].resource_id;
    let id = app.doc.canvases[0].objects[0].id;
    let p = app
        .plan_curve_series_order(
            DocumentRevision(app.doc.automation_revision),
            canvas,
            id,
            order,
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    assert_eq!(target(&app, 0, 0), second);
    assert_eq!(plot(&app, 0).binding.coordinates, before.coordinates);
    assert_eq!(plot(&app, 0).axis_overrides, overrides);
    app.undo();
    assert_eq!(target(&app, 0, 0), first);
    app.redo();
    assert_eq!(target(&app, 0, 0), second);
}

#[path = "display_field_tests.rs"]
mod field;
#[path = "display_marker_tests.rs"]
mod marker;
