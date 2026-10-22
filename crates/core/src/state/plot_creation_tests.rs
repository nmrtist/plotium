use super::*;
use crate::automation::*;
use plotium_data::{ColumnId, RevisionId};
use plotium_figure::SeriesKind;

fn fixture() -> PlotiumApp {
    let mut app = super::curve_tests::fixture(false);
    app.doc.canvases.clear();
    app.session.active_canvas = None;
    app.clear_history();
    app
}
fn source(app: &PlotiumApp, x: usize, y: usize, error: Option<usize>) -> TableCurveSource {
    let dataset = &app.doc.datasets[0];
    let t = dataset.as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[x].id,
        y_column_id: c[y].id,
        y_error_column_id: error.map(|i| c[i].id),
    }
}
fn params(app: &PlotiumApp) -> PlotCreateParams {
    PlotCreateParams {
        parent: document_root_target(),
        name: "Native line".into(),
        canvas: Some(NewPlotCanvas {
            name: "R5 native".into(),
            size_mm: Some([120., 80.]),
            size_preset: None,
        }),
        panel_name: None,
        panel_frame_pt: None,
        series: vec![NewCurve {
            y_axis: Default::default(),
            kind: CurveKind::Line,
            source: source(app, 0, 1, Some(2)),
        }],
    }
}
fn plan(app: &PlotiumApp, p: &PlotCreateParams) -> CurveEditPlan {
    app.plan_plot_creation(DocumentRevision(app.doc.automation_revision), p)
        .unwrap()
}
fn plot(app: &PlotiumApp) -> &PlotObject {
    app.doc.canvases[0].objects[0].plot().unwrap()
}
#[test]
fn creates_native_line_and_errors_atomically_undo_redo_keep_exact_ids() {
    let mut app = fixture();
    let p = params(&app);
    let t = app.doc.datasets[0].as_table().unwrap();
    let defaults = (t.x_binding, t.series_bindings.clone());
    let planned = plan(&app, &p);
    let summary = planned.creation.clone().unwrap();
    assert!(app.doc.canvases.is_empty());
    assert!(!app.can_undo());
    app.commit_curve_bindings(planned.clone()).unwrap();
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.doc.canvases[0].panels.len(), 1);
    assert!(plot(&app).display_owner.is_none());
    assert_eq!(plot(&app).figure().series[0].kind, SeriesKind::Line);
    assert_eq!(
        plot(&app).figure().series[0].points,
        vec![[0., 1.], [1., 2.], [2., 4.]]
    );
    assert_eq!(plot(&app).figure().error_bars.len(), 3);
    assert_eq!(
        summary.series_defaults[0].target,
        planned.bindings[0].target
    );
    assert_eq!(
        summary.series_defaults[0].line_width_pt,
        plot(&app).figure().series[0].width
    );
    assert!(app.commit_curve_bindings(planned).is_err());
    app.undo();
    assert!(app.doc.canvases.is_empty());
    assert_eq!(app.doc.datasets.len(), 1);
    app.redo();
    let c = &app.doc.canvases[0];
    let o = &c.objects[0];
    assert_eq!(summary.canvas, TargetRef::resource(c.resource_id.into()));
    assert_eq!(
        summary.panel,
        TargetRef::resource(panel_resource(c, &c.panels[0]))
    );
    assert_eq!(
        summary.plot,
        TargetRef::resource(canvas_object_ref(c.resource_id, o.id))
    );
    assert_eq!(
        summary.series_defaults[0].target,
        app.series_target(0, o.id, plot(&app).binding.series[0].id)
            .unwrap()
    );
    let t = app.doc.datasets[0].as_table().unwrap();
    assert!((t.x_binding, t.series_bindings.clone()) == defaults);
}
#[test]
fn invalid_sources_units_errors_and_stale_revisions_leave_no_containers_or_history() {
    let app = fixture();
    let good = params(&app);
    let mut cases = Vec::new();
    let mut p = good.clone();
    p.panel_frame_pt = Some([0., 0., 0., 100.]);
    cases.push(p);
    let mut p = good.clone();
    p.series[0].source.y_column_id = ColumnId::new();
    cases.push(p);
    let mut p = good.clone();
    p.series[0].source.table_revision = RevisionId::new();
    cases.push(p);
    let mut p = good.clone();
    p.series[0].source.field_id = FieldId::new(999);
    cases.push(p);
    let mut p = good.clone();
    p.series[0].source.dataset_id = DatasetId::new();
    cases.push(p);
    let mut p = good.clone();
    p.series[0].source.y_error_column_id = Some(p.series[0].source.x_column_id);
    cases.push(p);
    let mut p = good.clone();
    p.series.push(NewCurve {
        y_axis: Default::default(),
        kind: CurveKind::Scatter,
        source: source(&app, 0, 1, Some(5)),
    });
    cases.push(p);
    let mut p = good.clone();
    p.series.push(NewCurve {
        y_axis: Default::default(),
        kind: CurveKind::Scatter,
        source: source(&app, 3, 4, None),
    });
    cases.push(p);
    let mut p = good.clone();
    p.parent.resource.parent_id = Some("forged".into());
    cases.push(p);
    let mut p = good.clone();
    p.canvas.as_mut().unwrap().size_mm = Some([f32::NAN, 80.]);
    cases.push(p);
    let mut p = good.clone();
    p.series.clear();
    cases.push(p);
    for p in cases {
        assert!(app.plan_plot_creation(DocumentRevision(0), &p).is_err());
    }
    assert!(app.plan_plot_creation(DocumentRevision(99), &good).is_err());
    assert!(app.doc.canvases.is_empty());
    assert!(!app.can_undo());
    assert_eq!(app.doc.automation_revision, 0);
}
#[test]
fn commit_rechecks_table_revision_and_parent_state_without_partial_creation() {
    let mut app = fixture();
    let p = params(&app);
    let planned = plan(&app, &p);
    app.doc.datasets[0]
        .as_table_mut()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .id = RevisionId::new();
    assert!(app.commit_curve_bindings(planned).is_err());
    assert!(app.doc.canvases.is_empty());
    assert!(!app.can_undo());
    let mut app = fixture();
    app.doc
        .canvases
        .push(CanvasDocument::new("Existing".into(), [120., 80.]));
    let mut p = params(&app);
    p.canvas = None;
    p.parent = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    let planned = plan(&app, &p);
    app.doc.canvases[0].create_panel("intervening".into(), ObjectFrame::new(0., 0., 100., 100.));
    assert!(app.commit_curve_bindings(planned).is_err());
    assert!(app.doc.canvases[0].objects.is_empty());
    assert!(!app.can_undo());
}
#[test]
fn existing_canvas_and_panel_keep_containers_and_allocator_high_water_on_undo() {
    for existing_panel in [false, true] {
        let mut app = fixture();
        let mut c = CanvasDocument::new("Authored".into(), [120., 80.]);
        c.caption = "Keep this".into();
        let pid = if existing_panel {
            Some(c.create_panel(
                "Authored panel".into(),
                ObjectFrame::new(12., 24., 200., 150.),
            ))
        } else {
            None
        };
        let mut p = params(&app);
        p.canvas = None;
        p.parent = pid.map_or_else(
            || TargetRef::resource(c.resource_id.into()),
            |id| TargetRef::resource(panel_resource(&c, c.panel(id).unwrap())),
        );
        app.doc.canvases.push(c);
        let planned = plan(&app, &p);
        let first = planned.creation.clone().unwrap();
        app.commit_curve_bindings(planned).unwrap();
        let high = app.doc.canvases[0].next_object_id;
        app.undo();
        let c = &app.doc.canvases[0];
        assert!(c.objects.is_empty());
        assert_eq!(c.panels.len(), usize::from(existing_panel));
        assert_eq!(c.next_object_id, high);
        assert_eq!(c.caption, "Keep this");
        app.redo();
        assert_eq!(
            app.doc.canvases[0].panels[0].id.to_string(),
            first.panel.resource.local_id.clone().unwrap()
        );
        app.undo();
        let later = plan(&app, &p);
        assert_ne!(later.creation.as_ref().unwrap().plot, first.plot);
        app.commit_curve_bindings(later).unwrap();
        assert_eq!(app.doc.canvases[0].caption, "Keep this");
        if let Some(pid) = pid {
            assert_eq!(app.doc.canvases[0].panels[0].id, pid);
            assert_eq!(app.doc.canvases[0].panels[0].frame.x, 12.);
        }
    }
}
#[test]
fn multiple_explicit_series_scatter_and_compatible_units_share_native_coordinates() {
    let mut app = fixture();
    let mut p = params(&app);
    p.series.push(NewCurve {
        y_axis: Default::default(),
        kind: CurveKind::Scatter,
        source: source(&app, 7, 6, Some(8)),
    });
    let planned = plan(&app, &p);
    app.commit_curve_bindings(planned).unwrap();
    let f = plot(&app).figure();
    assert_eq!(f.series.len(), 2);
    assert_eq!(f.series[1].kind, SeriesKind::Points);
    assert_eq!(f.series[1].points[1], [1., 4.]);
    assert_eq!(f.error_bars[4].positive, 0.2);
    assert_ne!(
        plot(&app).binding.series[0].id,
        plot(&app).binding.series[1].id
    );
}
#[test]
fn creation_is_discoverable_previewable_and_v1_roundtrips() {
    let mut app = fixture();
    let p = params(&app);
    let planned = plan(&app, &p);
    let summary = planned.creation.clone().unwrap();
    app.commit_curve_bindings(planned).unwrap();
    for target in summary.created {
        let req = ToolRequest {
            tool_id: DOCUMENT_INSPECT.into(),
            tool_version: 1,
            parameters: serde_json::json!({"target":target,"view":"self"}),
            targets: if target.component.is_some() {
                TargetSelector::Components {
                    targets: vec![target.clone()],
                }
            } else {
                TargetSelector::Explicit {
                    ids: vec![target.resource.id.clone()],
                }
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Agent,
        };
        let prepared = plan_tool(&app, req).unwrap();
        let r = execute_tool(&mut app, prepared, ExecutionAuthority::Read);
        assert!(r.is_ok());
    }
    let path = std::env::temp_dir().join(format!(
        "plotium-r5-roundtrip-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        loaded.doc.canvases[0].resource_id,
        app.doc.canvases[0].resource_id
    );
    assert_eq!(loaded.doc.canvases[0].panels, app.doc.canvases[0].panels);
    assert_eq!(plot(&loaded).binding, plot(&app).binding);
    assert_eq!(
        plot(&loaded).figure().series[0].points,
        plot(&app).figure().series[0].points
    );
}
#[test]
fn native_creation_plans_cannot_be_reconstituted_from_json() {
    let mut app = fixture();
    let planned = plan(&app, &params(&app));
    let wire = serde_json::to_value(planned).unwrap();
    let untrusted: CurveEditPlan = serde_json::from_value(wire).unwrap();
    assert!(app.commit_curve_bindings(untrusted).is_err());
    assert!(app.doc.canvases.is_empty());
}

#[test]
fn existing_nonzero_margins_use_gui_native_placement_and_resizing_preserves_binding() {
    let mut app = fixture();
    let mut page = CanvasDocument::new("Margins".into(), [120., 80.]);
    page.layout.margin_mm = [3., 5., 7., 11.];
    let layout = page.layout;
    let mut p = params(&app);
    p.canvas = None;
    p.parent = TargetRef::resource(page.resource_id.into());
    app.doc.canvases.push(page);
    let prepared = plan(&app, &p);
    app.commit_curve_bindings(prepared).unwrap();
    let c = &app.doc.canvases[0];
    let object = &c.objects[0];
    let panel = &c.panels[0];
    let gui = crate::layout::compute_tiling_plan_for_items(
        c.size_pt(),
        &layout,
        &[],
        crate::layout::layout_item(object.id, object.plot().unwrap().figure(), object.frame),
        [0., 0.],
    )
    .newcomer;
    assert_eq!(panel.frame, gui);
    assert!((panel.frame.x - 11. * MM_TO_PT).abs() < 1e-4);
    assert!((panel.frame.y - 3. * MM_TO_PT).abs() < 1e-4);
    assert!((panel.frame.width - 104. * MM_TO_PT).abs() < 1e-4);
    assert!((panel.frame.height - 70. * MM_TO_PT).abs() < 1e-4);
    assert_eq!(object.frame.x, 0.);
    assert_eq!(object.frame.y, 0.);
    let before = plot(&app).binding.clone();
    let id = object.id;
    let cid = c.resource_id;
    let pid = panel.id;
    app.execute_action(crate::actions::Action::set_object_frames(
        0,
        vec![(id, panel.frame)],
        vec![(id, ObjectFrame::new(18., 25., 260., 160.))],
    ));
    assert_eq!(app.doc.canvases[0].resource_id, cid);
    assert_eq!(app.doc.canvases[0].panels[0].id, pid);
    assert_eq!(app.doc.canvases[0].objects[0].id, id);
    assert_eq!(plot(&app).binding, before);
    assert_eq!(
        app.doc.canvases[0].objects[0].frame,
        ObjectFrame::new(0., 0., 260., 160.)
    );
    app.undo();
    assert_eq!(plot(&app).binding, before);
    assert_eq!(app.doc.canvases[0].panels[0].frame, gui);
}

#[test]
fn auto_y_includes_visible_error_endpoints_and_retains_manual_ranges() {
    let mut app = fixture();
    let prepared = plan(&app, &params(&app));
    app.commit_curve_bindings(prepared).unwrap();
    let f = plot(&app).figure();
    let full = visible_y_range(f, AxisRange::new(0., 2.)).unwrap();
    assert!(full.min < 0.9 && full.max > 4.3);
    let limited = visible_y_range(f, AxisRange::new(0., 1.)).unwrap();
    assert!(limited.min < 0.9 && limited.max > 2.2 && limited.max < 3.);
    let mut viewport = plot(&app).viewport.clone();
    viewport.auto_y = false;
    viewport.view_y = AxisRange::new(1., 3.);
    viewport.sync_full_from(f);
    assert_eq!(viewport.view_y, AxisRange::new(1., 3.));
}

#[test]
fn constant_curves_keep_readable_auto_y_on_creation_and_rebuild() {
    use plotium_data::{ColumnChunk, ColumnSchema, LogicalType};

    for kind in [CurveKind::Line, CurveKind::Scatter] {
        for value in [0., 1., 100., -100.] {
            for count in [1, 3] {
                for zero_error in [false, true] {
                    let mut app = fixture();
                    let columns = [
                        ("x", (0..count).map(|i| i as f64).collect::<Vec<_>>()),
                        ("y", vec![value; count]),
                        ("error", vec![0.; count]),
                    ]
                    .into_iter()
                    .map(|(name, values)| {
                        (
                            ColumnSchema::new(name, LogicalType::Float64),
                            ColumnChunk::optional_f64(values.into_iter().map(Some)),
                        )
                    })
                    .collect();
                    app.doc.datasets[0] = Dataset::Table(Box::new(
                        TableDataset::from_materialized(
                            columns,
                            vec![],
                            None,
                            vec![],
                            "plotium.test.r5.constant",
                        )
                        .unwrap(),
                    ));
                    let mut p = params(&app);
                    p.series[0].kind = kind;
                    p.series[0].source = source(&app, 0, 1, zero_error.then_some(2));
                    app.commit_curve_bindings(plan(&app, &p)).unwrap();
                    let created = plot(&app);
                    let range = created.viewport.view_y;
                    assert!(range.is_valid(), "{kind:?} Y={value}, count={count}");
                    assert!(range.min < value && value < range.max);
                    assert!(range.span() >= value.abs().max(1.) * 0.1);
                    assert!((0.1..0.9).contains(&created.figure().y.normalize(value)));
                    assert_eq!(AxisRange::from_axis(&created.figure().y), range);
                    assert_eq!(
                        created.figure().error_bars.len(),
                        if zero_error { count } else { 0 }
                    );
                    let binding = created.binding.clone();
                    app.rebuild_canvas(0);
                    assert_eq!(plot(&app).viewport.view_y, range);
                    assert_eq!(plot(&app).binding, binding);
                    app.undo();
                    app.redo();
                    assert_eq!(plot(&app).viewport.view_y, range);
                    assert_eq!(plot(&app).binding, binding);
                }
            }
        }
    }
}

#[test]
fn undo_redo_new_canvas_retains_ids_minted_by_later_undone_gui_content() {
    let mut app = fixture();
    let planned = plan(&app, &params(&app));
    let identity = planned.creation.clone().unwrap();
    app.commit_curve_bindings(planned).unwrap();
    let old_id = app.doc.canvases[0].allocate_object_id();
    app.execute_action(crate::actions::Action::InsertObject {
        canvas: 0,
        object: Box::new(CanvasObject {
            id: old_id,
            name: "Later authored text".into(),
            frame: ObjectFrame::new(10., 10., 50., 20.),
            locked: false,
            visible: true,
            kind: CanvasObjectKind::Text(TextBox::label("note".into())),
        }),
        selection_before: Selection::None,
    });
    app.undo();
    app.undo();
    assert!(app.doc.canvases.is_empty());
    app.redo();
    assert_eq!(
        TargetRef::resource(app.doc.canvases[0].resource_id.into()),
        identity.canvas
    );
    assert_ne!(app.doc.canvases[0].allocate_object_id(), old_id);
}

#[test]
fn explicit_new_panel_on_existing_plot_preserves_first_and_ids_through_undo_redo() {
    let mut app = fixture();
    let p = params(&app);
    let planned = plan(&app, &p);
    app.commit_curve_bindings(planned).unwrap();
    let before = app.doc.canvases[0].clone();
    let mut p = params(&app);
    p.parent = TargetRef::resource(before.resource_id.into());
    p.canvas = None;
    p.panel_frame_pt = Some([260., 10., 230., 200.]);
    p.series[0].kind = CurveKind::Scatter;
    let planned = plan(&app, &p);
    let created = planned.creation.as_ref().unwrap().clone();
    app.commit_curve_bindings(planned).unwrap();
    let c = &app.doc.canvases[0];
    assert_eq!(c.panels[0], before.panels[0]);
    assert_eq!(c.objects[0].frame, before.objects[0].frame);
    assert_eq!(
        c.objects[0].plot().unwrap().binding,
        before.objects[0].plot().unwrap().binding
    );
    assert_eq!(c.panels.len(), 2);
    app.undo();
    assert_eq!(app.doc.canvases[0].panels, before.panels);
    app.redo();
    assert_eq!(
        created.plot,
        TargetRef::resource(canvas_object_ref(
            before.resource_id,
            app.doc.canvases[0].objects[1].id
        ))
    );
}

#[test]
fn append_selected_cross_source_curves_preserves_existing_identity_style_and_range() {
    let mut app = fixture();
    let p = params(&app);
    app.commit_curve_bindings(plan(&app, &p)).unwrap();
    let table = materialized_float_series_table(
        (
            "x_s".into(),
            "ms".into(),
            vec![Some(0.), Some(1000.), Some(2000.)],
        ),
        vec![FloatSeries {
            name: "y_mA".into(),
            unit: "A".into(),
            values: vec![Some(0.002), Some(0.004), Some(0.008)],
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.append",
    )
    .unwrap();
    let mut table = table;
    table.name = app.doc.datasets[0].as_table().unwrap().name.clone();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let dataset = &app.doc.datasets[1];
    let r = &dataset.as_table().unwrap().typed_state.envelope.revision;
    let added_source = TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: r.snapshot.schema.columns[0].id,
        y_column_id: r.snapshot.schema.columns[1].id,
        y_error_column_id: None,
    };
    let object = app.doc.canvases[0].objects[0].id;
    let owner = TargetRef::resource(canvas_object_ref(app.doc.canvases[0].resource_id, object));
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    plot.axis_overrides.x_range = Some(AxisRange::new(0., 3.));
    plot.viewport.view_x = AxisRange::new(0., 3.);
    let before = plot.clone();
    let geometry = format!(
        "{:?}{:?}",
        app.doc.canvases[0].panels, app.doc.canvases[0].objects[0].frame
    );
    let p = CurveAppendParams {
        plot: owner.clone(),
        series: vec![NewCurve {
            y_axis: Default::default(),
            kind: CurveKind::Line,
            source: added_source.clone(),
        }],
    };
    let plan = app
        .plan_append_curves(DocumentRevision(app.doc.automation_revision), &p)
        .unwrap();
    let appended = plan.appended.clone();
    assert_eq!(
        plan.bindings[1].source.dataset_id,
        app.doc.datasets[1].resource_id()
    );
    app.commit_curve_bindings(plan).unwrap();
    let after = app.doc.canvases[0].objects[0].plot().unwrap().clone();
    assert_eq!(after.binding.series[0], before.binding.series[0]);
    assert_eq!(after.binding.coordinates, before.binding.coordinates);
    assert_eq!(after.axis_overrides, before.axis_overrides);
    assert_eq!(after.viewport.view_x, before.viewport.view_x);
    assert_eq!(
        after.figure().series[1].points,
        vec![[0., 2.], [1., 4.], [2., 8.]]
    );
    assert_eq!(
        app.series_target(0, object, after.binding.series[1].id)
            .unwrap(),
        appended[0]
    );
    assert_eq!(
        geometry,
        format!(
            "{:?}{:?}",
            app.doc.canvases[0].panels, app.doc.canvases[0].objects[0].frame
        )
    );
    app.undo();
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        before.binding
    );
    app.redo();
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        after.binding
    );
    app.undo();
    let newer = app
        .plan_append_curves(DocumentRevision(app.doc.automation_revision), &p)
        .unwrap();
    assert_ne!(newer.appended, appended);
    app.commit_curve_bindings(newer).unwrap();
    assert!(app.curve_target(&appended[0]).is_err());
    let before = app.doc.canvases[0].objects[0].plot().unwrap().clone();
    let revision = app.doc.automation_revision;
    for bad in [
        source_with_bad_column(&added_source),
        source_with_bad_revision(&added_source),
        source(&app, 3, 4, None),
    ] {
        let mut mixed = p.clone();
        mixed.series.push(NewCurve {
            y_axis: Default::default(),
            kind: CurveKind::Scatter,
            source: bad,
        });
        assert!(
            app.plan_append_curves(DocumentRevision(revision), &mixed)
                .is_err()
        );
        let now = app.doc.canvases[0].objects[0].plot().unwrap();
        assert_eq!(now.binding, before.binding);
        assert_eq!(now.next_series_id, before.next_series_id);
        assert_eq!(app.doc.automation_revision, revision);
    }
}
fn source_with_bad_column(source: &TableCurveSource) -> TableCurveSource {
    let mut s = source.clone();
    s.y_column_id = ColumnId::new();
    s
}
fn source_with_bad_revision(source: &TableCurveSource) -> TableCurveSource {
    let mut s = source.clone();
    s.table_revision = RevisionId::new();
    s
}
#[test]
fn new_canvas_takes_a_size_preset_with_its_identity() {
    let mut app = fixture();
    let mut p = params(&app);
    p.canvas = Some(NewPlotCanvas {
        name: "Journal".into(),
        size_mm: None,
        size_preset: Some(NATURE_DOUBLE_COLUMN.id.into()),
    });
    let planned = plan(&app, &p);
    let summary = planned.creation.as_ref().unwrap();
    assert_eq!(summary.size_mm, NATURE_DOUBLE_COLUMN.size_mm());
    assert_eq!(
        summary.size_preset.as_deref(),
        Some(NATURE_DOUBLE_COLUMN.id)
    );
    app.commit_curve_bindings(planned).unwrap();
    let canvas = &app.doc.canvases[0];
    assert_eq!(canvas.size_mm, NATURE_DOUBLE_COLUMN.size_mm());
    assert_eq!(
        canvas.size_preset_id.as_deref(),
        Some(NATURE_DOUBLE_COLUMN.id)
    );
    for (size_mm, size_preset) in [
        (Some([89., 60.]), Some("nature-1col".to_owned())),
        (None, None),
        (None, Some("nature-9col".to_owned())),
    ] {
        p.canvas = Some(NewPlotCanvas {
            name: "Journal".into(),
            size_mm,
            size_preset,
        });
        assert!(
            app.plan_plot_creation(DocumentRevision(app.doc.automation_revision), &p)
                .is_err()
        );
    }
}
