use super::*;
use plotium_data::{ColumnValues, ScalarValue};

#[test]
fn curve_data_only_table_binds_without_assigning_shared_defaults() {
    let mut app = fixture(false);
    {
        let table = app.doc.datasets[0].as_table_mut().unwrap();
        table.x_binding = None;
        table.series_bindings.clear();
    }
    let id = plot(&app, 0).binding.series[0].id;
    bind(&mut app, 0, id, 3, 4, None);
    assert!(table(&app).x_binding.is_none());
    assert!(table(&app).series_bindings.is_empty());
    assert_eq!(plot(&app, 0).figure().series[0].points[1], [0.2, 3.]);
}
#[test]
fn curve_materialization_preserves_each_displayed_trace_and_retains_chosen_y_identity() {
    let mut app = fixture(true);
    let object = app.doc.canvases[0].objects[0].id;
    {
        let s = &mut app.doc.canvases[0].objects[0]
            .plot_mut()
            .unwrap()
            .binding
            .series[0];
        s.label = Some("Authored".into());
        if let SeriesEncoding::Line(l) = &mut s.encoding {
            l.scale = 3.;
            l.x_shift = plotium_figure::FiniteF64::new(2.).unwrap();
        }
    }
    app.rebuild_canvas(0);
    let before = plot(&app, 0).figure().clone();
    let id = plot(&app, 0).binding.series[0].id;
    let keep = table(&app).series_bindings[1].value_column;
    let p = app
        .plan_materialize_curves(
            DocumentRevision(0),
            &[MaterializeCurveEdit {
                target: target(&app, 0, id),
                retain_y_column_id: keep,
                table_revision: table(&app).typed_state.envelope.revision.id,
            }],
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    let after = plot(&app, 0);
    assert_eq!(after.binding.series[1].id, id);
    assert_eq!(
        after.binding.series[1]
            .source
            .columns()
            .unwrap()
            .y_column_id,
        keep
    );
    assert_eq!(before.series.len(), after.figure().series.len());
    for (before, after) in before.series.iter().zip(&after.figure().series) {
        assert_eq!(before.points, after.points);
        assert_eq!(before.color, after.color);
        assert_eq!(before.width, after.width);
        assert_eq!(before.name, after.name);
        assert_eq!(before.kind, after.kind);
    }
    assert_eq!(app.doc.canvases[0].objects[0].id, object);
    app.undo();
    assert!(plot(&app, 0).binding.coordinates.is_none());
    app.redo();
    assert_eq!(plot(&app, 0).binding.series[1].id, id);
}
#[test]
fn curve_allocator_exhaustion_is_an_error_without_partial_changes() {
    let mut app = fixture(true);
    let id = plot(&app, 0).binding.series[0].id;
    let y = table(&app).series_bindings[0].value_column;
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .next_series_id = SeriesId::new(u64::MAX);
    let result = app.plan_materialize_curves(
        DocumentRevision(0),
        &[MaterializeCurveEdit {
            target: target(&app, 0, id),
            retain_y_column_id: y,
            table_revision: table(&app).typed_state.envelope.revision.id,
        }],
    );
    assert!(result.unwrap_err().contains("exhausted"));
    assert!(!app.can_undo());
    assert_eq!(plot(&app, 0).binding.series.len(), 1);
}
#[test]
fn curve_all_rows_are_validated_beyond_old_sampling_and_record_batch_boundary() {
    let n = 70003;
    let mut error = vec![Some(0.1); n];
    error[65537] = Some(-0.1);
    let cols = [
        ("x", vec![Some(1.); n]),
        ("y", vec![None; n]),
        ("error", error),
    ]
    .into_iter()
    .map(|(name, v)| {
        let mut c = ColumnSchema::new(name, LogicalType::Float64);
        c.nullable = true;
        (c, ColumnChunk::optional_f64(v))
    })
    .collect::<Vec<_>>();
    let ids = cols.iter().map(|c| c.0.id).collect::<Vec<_>>();
    let store = std::sync::Arc::new(plotium_data::MemoryBlockStore::default());
    let codecs = plotium_data::CodecRegistry::with_arrow_ipc();
    let schema =
        plotium_data::TableSchema::new(cols.iter().map(|c| c.0.clone()).collect()).unwrap();
    let mut builder = plotium_data::SnapshotBuilder::new(
        plotium_data::TableId::new(),
        schema,
        store.as_ref(),
        &codecs,
    )
    .unwrap();
    for (start, end) in [(0, 65536), (65536, n)] {
        let rows = (start..end)
            .map(|_| plotium_data::RowId::new())
            .collect::<Vec<_>>();
        let values = cols
            .iter()
            .map(|(_, chunk)| {
                ColumnChunk::optional_f64((start..end).map(|row| match chunk.value(row).unwrap() {
                    ScalarValue::Float64(v) => Some(v),
                    ScalarValue::Null => None,
                    _ => panic!("numeric fixture"),
                }))
            })
            .collect::<Vec<_>>();
        builder.push_batch(&rows, &values).unwrap();
    }
    let snapshot = builder.finish().unwrap();
    assert_eq!(snapshot.batch_count(), 2);
    let t = TableDataset::from_typed(TypedTableState::imported(snapshot, store).unwrap());
    let c = TableCurveColumns {
        table_id: t.typed_state.envelope.revision.table_id,
        x_column_id: ids[0],
        y_column_id: ids[1],
        y_error_column_id: Some(ids[2]),
    };
    assert!(
        extract_table_curve(&t, c, &CurveCoordinates::default())
            .err()
            .unwrap()
            .contains("non-negative")
    );
}
#[test]
fn curve_affine_units_convert_centers_but_not_absolute_amplitudes() {
    let registry = UnitRegistry::plotium_v1();
    let c = Some(registry.resolve("°C").unwrap());
    let k = Some(registry.resolve("K").unwrap());
    assert!((convert_curve_value(25., &c, &k, false).unwrap() - 298.15).abs() < 1e-12);
    assert_eq!(convert_curve_value(2., &c, &k, true).unwrap(), 2.);
    assert!(convert_curve_value(1., &None, &k, false).is_err());
}
#[test]
fn curve_numeric_types_and_exact_integer_conversion_are_enforced() {
    let x = ColumnSchema::new("x", LogicalType::Int64);
    let y = ColumnSchema::new("y", LogicalType::Int64);
    let xi = x.id;
    let yi = y.id;
    let t = TableDataset::from_materialized(
        vec![
            (x, ColumnChunk::all_valid(ColumnValues::Int64(vec![0, 1]))),
            (
                y,
                ColumnChunk::all_valid(ColumnValues::Int64(vec![1, 9_007_199_254_740_993])),
            ),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.integer",
    )
    .unwrap();
    let c = TableCurveColumns {
        table_id: t.typed_state.envelope.revision.table_id,
        x_column_id: xi,
        y_column_id: yi,
        y_error_column_id: None,
    };
    assert!(
        extract_table_curve(&t, c, &CurveCoordinates::default())
            .err()
            .unwrap()
            .contains("exactly")
    );
    // Non-numeric data is rejected from schema, not parsed from displayed text.
    for (kind, values) in [
        (
            LogicalType::Utf8,
            ColumnValues::Utf8(vec!["1".into(), "2".into()]),
        ),
        (
            LogicalType::Boolean,
            ColumnValues::Boolean(vec![true, false]),
        ),
    ] {
        let x = ColumnSchema::new("x", LogicalType::Float64);
        let y = ColumnSchema::new("y", kind);
        let (xi, yi) = (x.id, y.id);
        let t = TableDataset::from_materialized(
            vec![
                (x, ColumnChunk::optional_f64([Some(0.), Some(1.)])),
                (y, ColumnChunk::all_valid(values)),
            ],
            vec![],
            None,
            vec![],
            "plotium.test.nonnumeric",
        )
        .unwrap();
        let c = TableCurveColumns {
            table_id: t.typed_state.envelope.revision.table_id,
            x_column_id: xi,
            y_column_id: yi,
            y_error_column_id: None,
        };
        assert!(extract_table_curve(&t, c, &CurveCoordinates::default()).is_err());
    }
    let mut app = fixture(false);
    let mut candidate = source(&app, 0, 1, None);
    candidate.y_column_id = ColumnId::new();
    assert!(
        app.plan_curve_bindings(
            DocumentRevision(0),
            &[CurveBindingEdit {
                target: target(&app, 0, plot(&app, 0).binding.series[0].id),
                source: candidate
            }]
        )
        .is_err()
    );
    assert_eq!(
        table(&app).typed_rows(1, &[]).unwrap().columns[0].values[0],
        ScalarValue::Float64(0.)
    );
    app.clear_history();
}
#[test]
fn curve_binding_protocol_requires_explicit_error_column_or_null() {
    let mut app = fixture(false);
    let id = plot(&app, 0).binding.series[0].id;
    let t = target(&app, 0, id);
    let mut raw = serde_json::to_value(source(&app, 0, 1, None)).unwrap();
    raw.as_object_mut().unwrap().remove("y_error_column_id");
    assert!(serde_json::from_value::<TableCurveSource>(raw).is_err());
    let req = ToolRequest {
        tool_id: CURVE_BIND.into(),
        tool_version: 1,
        parameters: json!({"edits":[{"target":t,"source":source(&app,0,1,None)}]}),
        targets: TargetSelector::Explicit { ids: vec![] },
        expected_revision: DocumentRevision(0),
        caller: CallerType::Agent,
    };
    let p = plan_tool(&app, req).unwrap();
    let deserialized: ToolPlan = serde_json::from_value(serde_json::to_value(p).unwrap()).unwrap();
    assert!(execute_tool(&mut app, deserialized, ExecutionAuthority::ReversibleModify).is_err());
    assert!(!app.can_undo());
}

#[test]
fn curve_nonfinite_error_rejects_even_when_xy_is_missing() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let cols = [("x", Some(0.)), ("y", None), ("error", Some(value))].map(|(name, value)| {
            let mut c = ColumnSchema::new(name, LogicalType::Float64);
            c.nullable = true;
            (c, ColumnChunk::optional_f64([value]))
        });
        let ids = cols.each_ref().map(|c| c.0.id);
        let t = TableDataset::from_materialized(
            cols.into(),
            vec![],
            None,
            vec![],
            "plotium.test.nonfinite_error",
        )
        .unwrap();
        let c = TableCurveColumns {
            table_id: t.typed_state.envelope.revision.table_id,
            x_column_id: ids[0],
            y_column_id: ids[1],
            y_error_column_id: Some(ids[2]),
        };
        assert!(extract_table_curve(&t, c, &CurveCoordinates::default()).is_err());
    }
}
