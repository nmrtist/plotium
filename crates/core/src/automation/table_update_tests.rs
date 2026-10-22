use super::*;
use plotium_data::*;

fn fixture(rows: usize) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let table = materialized_float_series_table(
        (
            "x_left".into(),
            "s".into(),
            (0..rows).map(|i| Some(i as f64)).collect(),
        ),
        vec![FloatSeries {
            name: "y_left".into(),
            unit: "mA".into(),
            values: (0..rows)
                .map(|i| Some(if i < 3 { [1., 2., 4.][i] } else { i as f64 }))
                .collect(),
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.r9.v1",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app
}
fn request(app: &PlotiumApp, index: usize, tool: &str, parameters: Value) -> ToolRequest {
    ToolRequest {
        tool_id: tool.into(),
        tool_version: 1,
        parameters,
        targets: TargetSelector::Explicit {
            ids: vec![app.doc.datasets[index].resource_id().to_string()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Human,
    }
}
fn patch(app: &PlotiumApp, index: usize, row: usize, value: f64) -> ToolRequest {
    let t = app.doc.datasets[index].as_table().unwrap();
    let column = t.typed_state.envelope.revision.snapshot.schema.columns[1].id;
    let row = t.typed_rows_page(row as u64, 1, &[column]).unwrap().row_ids[0];
    request(
        app,
        index,
        TABLE_PATCH,
        json!({"table_revision":t.typed_state.envelope.revision.id,"edits":[{"row_id":row,"column_id":column,"value":{"kind":"float64","value":value}}]}),
    )
}
fn run(app: &mut PlotiumApp, req: ToolRequest) -> std::result::Result<ToolResult, AutomationError> {
    let plan = plan_tool(app, req)?;
    let prepared =
        TableOperationSnapshot::capture(app, &plan.request)?.prepare(&AtomicBool::new(false))?;
    execute_prepared_table_operation(app, plan, prepared, ExecutionAuthority::ReversibleModify)
}
fn derive(app: &mut PlotiumApp, index: usize) -> usize {
    let table = app.doc.datasets[index].as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let mut col = ColumnSchema::new(format!("signal_x2_{index}"), LogicalType::Float64);
    col.unit = c[1].unit.clone();
    let root = Relation::ComputedColumn {
        input: Box::new(Relation::SnapshotRead(SnapshotRead {
            table: r.table_id,
            revision: r.id,
            fingerprint: r.snapshot.fingerprint,
        })),
        column: col,
        expression: Expression::call(
            "multiply.v1",
            vec![
                Expression::column(c[1].id),
                Expression::Literal {
                    value: LiteralValue::Float64(FiniteOrSpecial::new(2.)),
                },
            ],
        ),
    };
    let req = request(
        app,
        index,
        "data.transform",
        json!({"mode":"table_data_only","name":"Scaled","plan":RelPlanV1::new(root),"memory_limit_bytes":16777216}),
    );
    run(app, req).unwrap();
    app.doc.datasets.len() - 1
}
fn refresh(app: &PlotiumApp, index: usize) -> ToolRequest {
    request(
        app,
        index,
        TABLE_REFRESH,
        json!({"table_revision":app.doc.datasets[index].as_table().unwrap().typed_state.envelope.revision.id}),
    )
}
fn values(app: &PlotiumApp, index: usize, column: usize) -> Vec<f64> {
    let t = app.doc.datasets[index].as_table().unwrap();
    let id = t.typed_state.envelope.revision.snapshot.schema.columns[column].id;
    t.typed_rows(3, &[id]).unwrap().columns[0]
        .values
        .iter()
        .map(|v| {
            if let ScalarValue::Float64(v) = v {
                *v
            } else {
                panic!("float fixture")
            }
        })
        .collect()
}
fn status(app: &PlotiumApp, index: usize) -> TableFreshnessState {
    app.doc
        .table_freshness(app.doc.datasets[index].resource_id())
        .status
}
fn source(app: &PlotiumApp, index: usize, y: usize) -> TableCurveSource {
    let d = &app.doc.datasets[index];
    let t = d.as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    TableCurveSource {
        dataset_id: d.resource_id(),
        field_id: d.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[0].id,
        y_column_id: c[y].id,
        y_error_column_id: None,
    }
}
fn authored(app: &PlotiumApp) -> String {
    let c = &app.doc.canvases[0];
    format!(
        "{:?}",
        (
            c.resource_id,
            &c.panels,
            c.objects
                .iter()
                .map(|o| {
                    let p = o.plot().unwrap();
                    (
                        o.id,
                        o.frame,
                        &p.binding,
                        p.viewport.view_x,
                        &p.axis_overrides,
                        p.next_series_id,
                    )
                })
                .collect::<Vec<_>>()
        )
    )
}

#[test]
fn patch_refresh_undo_redo_preserve_native_curves_display_and_freshness() {
    let mut app = fixture(3);
    let derived = derive(&mut app, 0);
    let p = PlotCreateParams {
        parent: document_root_target(),
        name: "Native".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series: vec![
            NewCurve {
                y_axis: Default::default(),
                kind: CurveKind::Line,
                source: source(&app, 0, 1),
            },
            NewCurve {
                y_axis: Default::default(),
                kind: CurveKind::Line,
                source: source(&app, derived, 2),
            },
        ],
    };
    let plan = app
        .plan_plot_creation(DocumentRevision(app.doc.automation_revision), &p)
        .unwrap();
    app.commit_curve_bindings(plan).unwrap();
    let cid = app.doc.canvases[0].resource_id;
    let oid = app.doc.canvases[0].objects[0].id;
    let sid = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .series[0]
        .id;
    let target = app.series_target(0, oid, sid).unwrap();
    for (key, value, selector) in [
        (
            "series.line.color",
            json!("#D55E00"),
            TargetSelector::Components {
                targets: vec![target],
            },
        ),
        (
            "object.axes.x_range",
            json!({"mode":"manual","min":0,"max":3,"unit":"s"}),
            TargetSelector::Explicit {
                ids: vec![canvas_object_ref(cid, oid).id],
            },
        ),
    ] {
        let req = ToolRequest {
            tool_id: "properties.set".into(),
            tool_version: 1,
            parameters: json!({"key":key,"value":value}),
            targets: selector,
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Human,
        };
        let plan = plan_tool(&app, req).unwrap();
        execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    }
    let authored_before = authored(&app);
    let ids = app
        .doc
        .datasets
        .iter()
        .map(Dataset::resource_id)
        .collect::<Vec<_>>();
    let req = patch(&app, 0, 1, 2.5);
    run(&mut app, req).unwrap();
    assert_eq!(values(&app, 0, 1), [1., 2.5, 4.]);
    assert_eq!(values(&app, derived, 2), [2., 4., 8.]);
    assert_eq!(status(&app, derived), TableFreshnessState::Stale);
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series[0]
            .points[1],
        [1., 2.5]
    );
    app.undo();
    assert_eq!(status(&app, derived), TableFreshnessState::Current);
    assert_eq!(values(&app, 0, 1), [1., 2., 4.]);
    app.redo();
    assert_eq!(status(&app, derived), TableFreshnessState::Stale);
    let req = refresh(&app, derived);
    let result = run(&mut app, req).unwrap();
    assert!(result.produced.is_empty());
    assert_eq!(values(&app, derived, 2), [2., 5., 8.]);
    assert_eq!(status(&app, derived), TableFreshnessState::Current);
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series[1]
            .points[1],
        [1., 5.]
    );
    app.undo();
    assert_eq!(values(&app, derived, 2), [2., 4., 8.]);
    assert_eq!(status(&app, derived), TableFreshnessState::Stale);
    app.redo();
    assert_eq!(values(&app, derived, 2), [2., 5., 8.]);
    assert_eq!(status(&app, derived), TableFreshnessState::Current);
    assert_eq!(authored_before, authored(&app));
    assert_eq!(
        ids,
        app.doc
            .datasets
            .iter()
            .map(Dataset::resource_id)
            .collect::<Vec<_>>()
    );
}
#[test]
fn row_10001_atomic_errors_exact_type_noop_and_stale_revision() {
    let mut app = fixture(10002);
    let original = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .id;
    let req = patch(&app, 0, 10000, 10000.5);
    for fault in ["row", "column", "type", "duplicate", "revision", "unknown"] {
        let mut bad = req.clone();
        let edit = &mut bad.parameters["edits"][0];
        match fault {
            "row" => edit["row_id"] = json!(RowId::new()),
            "column" => edit["column_id"] = json!(ColumnId::new()),
            "type" => edit["value"] = json!({"kind":"utf8","value":"2.5"}),
            "duplicate" => {
                let e = edit.clone();
                bad.parameters["edits"].as_array_mut().unwrap().push(e);
            }
            "revision" => bad.parameters["table_revision"] = json!(RevisionId::new()),
            "unknown" => edit["extra"] = json!(true),
            _ => unreachable!(),
        }
        if fault != "duplicate" {
            bad.parameters["edits"]
                .as_array_mut()
                .unwrap()
                .insert(0, req.parameters["edits"][0].clone());
        }
        assert!(run(&mut app, bad).is_err(), "{fault}");
        assert_eq!(
            app.doc.datasets[0]
                .as_table()
                .unwrap()
                .typed_state
                .envelope
                .revision
                .id,
            original
        );
        assert!(!app.can_undo());
    }
    run(&mut app, req.clone()).unwrap();
    let t = app.doc.datasets[0].as_table().unwrap();
    let page = t
        .typed_rows_page(9999, 3, &[t.series_bindings[0].value_column])
        .unwrap();
    assert_eq!(
        page.columns[0].values,
        [
            ScalarValue::Float64(9999.),
            ScalarValue::Float64(10000.5),
            ScalarValue::Float64(10001.)
        ]
    );
    assert!(run(&mut app, req).is_err());
    let rev = app.doc.automation_revision;
    let req = patch(&app, 0, 10000, 10000.5);
    assert_eq!(run(&mut app, req).unwrap().value["changed"], false);
    assert_eq!(app.doc.automation_revision, rev);
}
#[test]
fn upstream_staleness_missing_source_and_repeat_rebase_are_inspectable() {
    let mut app = fixture(3);
    let d = derive(&mut app, 0);
    let dd = derive(&mut app, d);
    let req = patch(&app, d, 0, 10.);
    run(&mut app, req).unwrap();
    let req = patch(&app, 0, 1, 2.5);
    run(&mut app, req).unwrap();
    assert_eq!(status(&app, d), TableFreshnessState::Stale);
    assert_eq!(status(&app, dd), TableFreshnessState::Stale);
    assert!(
        !app.doc
            .table_freshness(app.doc.datasets[dd].resource_id())
            .refreshable
    );
    let req = refresh(&app, dd);
    assert!(
        run(&mut app, req)
            .unwrap_err()
            .to_string()
            .contains("Refresh source")
    );
    for _ in 0..2 {
        let req = refresh(&app, d);
        run(&mut app, req).unwrap();
        assert_eq!(values(&app, d, 1), [10., 2.5, 4.]);
        assert_eq!(status(&app, d), TableFreshnessState::Current);
    }
    let req = refresh(&app, dd);
    run(&mut app, req).unwrap();
    assert_eq!(status(&app, dd), TableFreshnessState::Current);
    let page = super::super::table_provenance::row_sources(
        app.doc.datasets[d].as_table().unwrap(),
        0,
        &app.doc.datasets[d]
            .as_table()
            .unwrap()
            .typed_rows(3, &[])
            .unwrap()
            .row_ids,
    )
    .unwrap();
    assert_eq!(page.len(), 3);
    app.doc.datasets.remove(0);
    let f = app.doc.table_freshness(app.doc.datasets[0].resource_id());
    assert_eq!(f.status, TableFreshnessState::Unavailable);
    assert!(f.reason.contains("missing"));
}
#[test]
fn analysis_invalidation_and_refresh_undo_share_sheet_commit_semantics() {
    let mut app = fixture(3);
    let d = derive(&mut app, 0);
    let fit = CurveFitReference {
        analysis_id: 7,
        instance_id: "series".into(),
        response: "y".into(),
    };
    app.doc.datasets[0].as_table_mut().unwrap().series_bindings[0].fit = Some(fit.clone());
    let req = patch(&app, 0, 1, 2.5);
    run(&mut app, req).unwrap();
    assert!(
        app.doc.datasets[0].as_table().unwrap().series_bindings[0]
            .fit
            .is_none()
    );
    app.undo();
    assert!(app.doc.datasets[0].as_table().unwrap().series_bindings[0].fit == Some(fit));
    app.redo();
    // Native GUI and MCP refresh both call commit_table_revision and retain the old revision in Undo.
    app.refresh_derived_table(d, &[0], 16777216).unwrap();
    assert_eq!(values(&app, d, 2), [2., 5., 8.]);
    app.undo();
    assert_eq!(values(&app, d, 2), [2., 4., 8.]);
    assert_eq!(status(&app, d), TableFreshnessState::Stale);
}

#[test]
fn native_refresh_completion_rejects_context_revision_and_cancel_without_adoption() {
    for mode in 0..3 {
        let mut app = fixture(3);
        let derived = derive(&mut app, 0);
        let req = patch(&app, 0, 1, 2.5);
        run(&mut app, req).unwrap();
        let before = app.doc.datasets[derived]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .id;
        let input = app.doc.datasets[0].resource_id();
        app.start_table_refresh(derived, vec![input], 16777216)
            .unwrap();
        match mode {
            0 => app.session.project_context = ProjectContextId::default(),
            1 => app.doc.automation_revision += 1,
            _ => {
                assert!(app.cancel_table_transform());
            }
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while app.session.table_refresh_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_table_transform();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(app.session.table_refresh_job.is_none());
        assert_eq!(
            app.doc.datasets[derived]
                .as_table()
                .unwrap()
                .typed_state
                .envelope
                .revision
                .id,
            before
        );
        assert_eq!(values(&app, derived, 2), [2., 4., 8.]);
        assert_eq!(status(&app, derived), TableFreshnessState::Stale);
        assert!(
            app.session
                .status
                .contains(if mode == 2 { "cancelled" } else { "discarded" })
        );
    }
}

#[test]
fn native_pivot_refresh_is_rejected_atomically_without_restricting_native_refresh() {
    let mut app = fixture(3);
    let input = TypedTableState::materialized(
        vec![
            materialized_float_column("x_left", "s", [Some(0.), Some(1.), Some(2.)]),
            materialized_float_column("y_left", "mA", [Some(1.), Some(2.), Some(4.)]),
            (
                ColumnSchema::new("channel", LogicalType::Utf8),
                ColumnChunk::all_valid(ColumnValues::Utf8(vec!["signal".into(); 3])),
            ),
        ],
        vec![],
        "plotium.test.pivot.v1",
    )
    .unwrap();
    app.doc.datasets[0] = Dataset::Table(Box::new(TableDataset::from_typed(input)));
    let input = &app.doc.datasets[0].as_table().unwrap().typed_state;
    let r = &input.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let plan = RelPlanV1::new(Relation::Pivot {
        input: Box::new(Relation::SnapshotRead(SnapshotRead {
            table: r.table_id,
            revision: r.id,
            fingerprint: r.snapshot.fingerprint,
        })),
        groups: vec![c[0].id],
        names_from: c[2].id,
        values_from: c[1].id,
        aggregate: AggregateFunction::SumV1,
    });
    let typed =
        execute_typed_plan(plan, &[input], TableId::new(), 16777216, &BTreeSet::new()).unwrap();
    // Native GUI recipes are broader than the connection's approved subset.
    let native = refresh_typed_plan(&typed, &[input], 16777216, &BTreeSet::new()).unwrap();
    assert_ne!(
        typed.envelope.revision.snapshot.schema.columns[1].id,
        native.envelope.revision.snapshot.schema.columns[1].id
    );
    let mut table = TableDataset::from_typed(typed);
    table.lineage = Some(DatasetLineage::new(
        DerivationKind::RelationalTransform,
        [app.doc.datasets[0].resource_id()],
    ));
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let p = PlotCreateParams {
        parent: document_root_target(),
        name: "Pivot curve".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series: vec![NewCurve {
            y_axis: Default::default(),
            kind: CurveKind::Line,
            source: source(&app, 1, 1),
        }],
    };
    let plan = app
        .plan_plot_creation(DocumentRevision(app.doc.automation_revision), &p)
        .unwrap();
    app.commit_curve_bindings(plan).unwrap();
    let authored_before = authored(&app);
    let typed_before = app.doc.datasets[1]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .clone();
    let revision = app.doc.automation_revision;
    let undo = app.session.undo_stack.len();
    assert!(
        app.doc
            .table_freshness(app.doc.datasets[1].resource_id())
            .refreshable
    );
    let id = app.doc.datasets[1].resource_id();
    let metadata = ProjectResourceProvider::new(&app)
        .inspect(&id.to_string())
        .unwrap()
        .metadata;
    assert_eq!(metadata["table_refreshable"], "true");
    assert_eq!(metadata["table_mcp_refreshable"], "false");
    assert!(metadata["table_mcp_refresh_reason"].contains("MCP refresh"));
    for tool in [TABLE_COLUMNS, TABLE_READ_ROWS] {
        let req = request(
            &app,
            1,
            tool,
            json!({"dataset_id":id,"table_revision":typed_before.revision.id}),
        );
        let plan = plan_tool(&app, req).unwrap();
        let read = execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
        assert_eq!(read.value["freshness"]["refreshable"], true);
        assert_eq!(read.value["freshness"]["mcp_refreshable"], false);
    }
    let req = refresh(&app, 1);
    let error = run(&mut app, req).unwrap_err().to_string();
    assert!(error.contains("MCP refresh"), "{error}");
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(app.session.undo_stack.len(), undo);
    assert_eq!(authored(&app), authored_before);
    assert_eq!(
        app.doc.datasets[1].as_table().unwrap().typed_state.envelope,
        typed_before
    );
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series[0]
            .points,
        vec![[0., 1.], [1., 2.], [2., 4.]]
    );
    app.undo(); // The rejected refresh must not hide the preceding plot creation.
    assert!(app.doc.canvases.is_empty());
    app.redo();
    assert_eq!(authored(&app), authored_before);
}

#[test]
fn refresh_filter_can_gain_or_lose_rows_and_sort_preserves_source_ids() {
    let mut app = fixture(3);
    let input = &app.doc.datasets[0].as_table().unwrap().typed_state;
    let r = &input.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let plan = RelPlanV1::new(Relation::Project {
        columns: c.iter().map(|c| c.id).collect(),
        input: Box::new(Relation::StableSort {
            input: Box::new(Relation::Filter {
                input: Box::new(Relation::SnapshotRead(SnapshotRead {
                    table: r.table_id,
                    revision: r.id,
                    fingerprint: r.snapshot.fingerprint,
                })),
                predicate: Expression::call(
                    "eq.v1",
                    vec![
                        Expression::column(c[1].id),
                        Expression::Literal {
                            value: LiteralValue::Float64(FiniteOrSpecial::new(2.)),
                        },
                    ],
                ),
            }),
            keys: vec![SortKey {
                column: c[0].id,
                direction: SortDirection::Descending,
                nulls: NullPlacement::Last,
            }],
        }),
    });
    let original_rows = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_rows(3, &[])
        .unwrap()
        .row_ids;
    let req = request(
        &app,
        0,
        "data.transform",
        json!({"mode":"table_data_only","name":"Filtered","plan":plan}),
    );
    run(&mut app, req).unwrap();
    let before = app.doc.datasets[1]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .clone();
    for (row, value, expected) in [
        (0, 2., vec![original_rows[1], original_rows[0]]),
        (1, 3., vec![original_rows[0]]),
    ] {
        let req = patch(&app, 0, row, value);
        run(&mut app, req).unwrap();
        let freshness = json!(connection_table_freshness(
            &app,
            app.doc.datasets[1].resource_id()
        ));
        assert_eq!(freshness["mcp_refreshable"], true);
        let req = refresh(&app, 1);
        run(&mut app, req).unwrap();
        let table = app.doc.datasets[1].as_table().unwrap();
        assert_eq!(
            table.typed_state.envelope.revision.table_id,
            before.table_id
        );
        assert_eq!(
            table.typed_state.envelope.revision.snapshot.schema,
            before.snapshot.schema
        );
        assert_eq!(table.typed_rows(3, &[]).unwrap().row_ids, expected);
    }
}

#[test]
fn refresh_candidate_guard_rejects_table_column_contract_and_foreign_row_identity() {
    let mut app = fixture(3);
    let d = derive(&mut app, 0);
    let before = &app.doc.datasets[d].as_table().unwrap().typed_state;
    let inputs = vec![app.doc.datasets[0].as_table().unwrap().typed_state.clone()];
    let after = refresh_typed_plan(before, &[&inputs[0]], 16777216, &BTreeSet::new()).unwrap();
    refresh::validate_output(before, &after, &inputs, &AtomicBool::new(false)).unwrap();
    for fault in ["table", "column", "type", "unit", "row"] {
        let mut bad = after.clone();
        let r = &mut bad.envelope.revision;
        match fault {
            "table" => r.table_id = TableId::new(),
            "column" => r.snapshot.schema.columns[1].id = ColumnId::new(),
            "type" => r.snapshot.schema.columns[1].logical_type = LogicalType::Utf8,
            "unit" => r.snapshot.schema.columns[1].unit = None,
            "row" => {
                let codecs = CodecRegistry::with_arrow_ipc();
                let reader = SnapshotReader::new(&r.snapshot, bad.store.as_ref(), &codecs).unwrap();
                let batch = reader.read_batch(0, &[]).unwrap();
                let mut builder = SnapshotBuilder::new(
                    r.table_id,
                    r.snapshot.schema.clone(),
                    bad.store.as_ref(),
                    &codecs,
                )
                .unwrap();
                let mut rows = batch.row_ids;
                rows[0] = RowId::new();
                builder
                    .push_batch(
                        &rows,
                        &batch
                            .columns
                            .into_iter()
                            .map(|(_, c)| c)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
                r.snapshot = builder.finish().unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            refresh::validate_output(before, &bad, &inputs, &AtomicBool::new(false)).is_err(),
            "{fault}"
        );
    }
}
