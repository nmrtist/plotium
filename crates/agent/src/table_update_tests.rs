use crate::{Connection, Failure, PendingRequest, Session};
use plotium_core::{automation::*, data::*, state::*};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
fn fixture() -> (PlotiumApp, Session) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let table = materialized_float_series_table(
        ("x".into(), "".into(), vec![Some(0.), Some(1.)]),
        vec![FloatSeries {
            name: "y".into(),
            unit: "".into(),
            values: vec![Some(1.), Some(2.)],
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.r9.agent",
    )
    .unwrap();
    let r = &table.typed_state.envelope.revision;
    let a = json!({"tool_id":"data.transform","ids":[table.resource_id],"expected_project_context":app.session.project_context,"expected_revision":0,"parameters":{"mode":"table_data_only","name":"Derived","plan":RelPlanV1::new(Relation::SnapshotRead(SnapshotRead{table:r.table_id,revision:r.id,fingerprint:r.snapshot.fingerprint})),"memory_limit_bytes":16777216}});
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut s = Session::default();
    s.allow_modify = true;
    let p = s.call(&mut app, "plotium_plan", a).unwrap();
    s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    (app, s)
}
fn args(app: &PlotiumApp, tool: &str) -> Value {
    let index = usize::from(tool == TABLE_REFRESH);
    let table = app.doc.datasets[index].as_table().unwrap();
    let mut parameters = json!({"table_revision":table.typed_state.envelope.revision.id});
    if tool == TABLE_PATCH {
        let c = table.series_bindings[0].value_column;
        let row = table.typed_rows(2, &[c]).unwrap().row_ids[1];
        parameters["edits"] =
            json!([{"row_id":row,"column_id":c,"value":{"kind":"float64","value":2.5}}]);
    }
    json!({"tool_id":tool,"ids":[table.resource_id],"parameters":parameters,"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision})
}

fn open_cell_draft(app: &mut PlotiumApp) {
    let table = app.doc.datasets[0].as_table().unwrap();
    let column = table.series_bindings[0].value_column;
    let row = table.typed_rows(2, &[column]).unwrap().row_ids[1];
    let revision = &table.typed_state.envelope.revision;
    app.session.ui.table_cell_edit = Some(TableCellEditDraft {
        dataset: table.resource_id,
        table: revision.table_id,
        revision: revision.id,
        row,
        column,
        original: LiteralValue::Float64(FiniteOrSpecial::Finite(2.0)),
        original_text: "2".into(),
        text: "9.5".into(),
        error: None,
        take_focus: false,
    });
}

#[test]
fn cell_draft_blocks_prepared_writes_but_allows_committed_state_inspection() {
    let (mut app, mut session) = fixture();
    let arguments = args(&app, TABLE_PATCH);
    let plan = session
        .call(&mut app, "plotium_plan", arguments.clone())
        .unwrap();
    open_cell_draft(&mut app);
    let before = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .id;
    let revision = app.doc.automation_revision;
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]})
            )
            .unwrap_err()
            .code,
        "busy"
    );
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(
        app.doc.datasets[0]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .id,
        before
    );
    assert_eq!(app.session.ui.table_cell_edit.as_ref().unwrap().text, "9.5");
    assert_eq!(
        session
            .call(&mut app, "plotium_plan", arguments)
            .unwrap_err()
            .code,
        "busy"
    );
    let state = session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(state["availability"]["project_read"]["available"], true);
    assert_eq!(
        state["availability"]["project_modify"]["reason"]["code"],
        "busy"
    );
    assert!(!state["project"].is_null());
    let inspect = json!({"key":"document.figure.typography.tick_pt","ids":["document"],"expected_project_context":app.session.project_context,"expected_revision":revision});
    session
        .call(&mut app, "plotium_properties_inspect", inspect)
        .unwrap();
    let draft = app.session.ui.table_cell_edit.as_ref().unwrap();
    let read = json!({"expected_project_context":app.session.project_context,"expected_revision":revision,"dataset_id":draft.dataset,"table_revision":draft.revision});
    let rows = session
        .call(&mut app, "plotium_table_read_rows", read)
        .unwrap();
    assert_eq!(rows["value"]["rows"][1]["values"][1]["value"], 2.0);

    app.session.ui.table_cell_edit = None;
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]})
            )
            .unwrap_err()
            .code,
        "expired_plan"
    );
    let arguments = args(&app, TABLE_PATCH);
    let plan = session.call(&mut app, "plotium_plan", arguments).unwrap();
    session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_ne!(
        app.doc.datasets[0]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .id,
        before
    );
}

#[test]
fn committing_cell_draft_invalidates_a_plan_prepared_before_editing() {
    let (mut app, mut session) = fixture();
    let arguments = args(&app, TABLE_PATCH);
    let plan = session.call(&mut app, "plotium_plan", arguments).unwrap();
    open_cell_draft(&mut app);
    let draft = app.session.ui.table_cell_edit.as_ref().unwrap();
    let table = app
        .doc
        .dataset_by_id(draft.dataset)
        .unwrap()
        .as_table()
        .unwrap();
    let after = prepare_table_cells(
        &table.typed_state,
        &[CellPatch {
            row: draft.row,
            column: draft.column,
            value: LiteralValue::Float64(FiniteOrSpecial::Finite(9.5)),
        }],
    )
    .unwrap();
    let (dataset, expected) = (draft.dataset, draft.revision);
    app.commit_table_revision(dataset, expected, after).unwrap();
    app.session.ui.table_cell_edit = None;
    let revision = app
        .doc
        .dataset_by_id(dataset)
        .unwrap()
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .id;
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]})
            )
            .unwrap_err()
            .code,
        "stale_revision"
    );
    assert_eq!(
        app.doc
            .dataset_by_id(dataset)
            .unwrap()
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .id,
        revision
    );
    let arguments = args(&app, TABLE_PATCH);
    let fresh = session.call(&mut app, "plotium_plan", arguments).unwrap();
    session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":fresh["plan_id"]}),
        )
        .unwrap();
}

#[test]
fn cell_draft_allows_preview_and_background_diagnostic_delivery() {
    let (mut app, mut session) = fixture();
    let mut canvas = CanvasDocument::new("Figure".into(), [100.0, 50.0]);
    let id = canvas.allocate_object_id();
    canvas.objects.push(ContentItem {
        id,
        name: "Outside".into(),
        frame: ObjectFrame::new(500.0, 500.0, 40.0, 20.0),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label("Outside".into())),
    });
    let target = TargetRef::resource(canvas.resource_id.into());
    app.doc.canvases.push(canvas);
    open_cell_draft(&mut app);
    let before = (
        app.doc.automation_revision,
        app.doc.edit_generation,
        app.session.undo_stack.len(),
    );
    let preview = json!({"expected_project_context":app.session.project_context,"expected_revision":before.0,"target":target,"max_edge":128});
    let diagnostic = json!({"expected_project_context":app.session.project_context,"expected_revision":before.0,"canvas":target,"output":{"size_mm":[100.0,50.0]}});
    let image = session
        .call(&mut app, "plotium_render_preview", preview.clone())
        .unwrap();
    assert_eq!(image["value"]["mime_type"], "image/png");
    assert_eq!(image["value"]["render_revision"], before.0);

    let mut connection = Connection::start(|| {}).unwrap();
    for edit_during_capture in [true, false] {
        if !edit_during_capture {
            app.session.ui.table_cell_edit = None;
        }
        let (response, receive) = tokio::sync::oneshot::channel();
        let request = PendingRequest {
            name: "plotium_figure_diagnose".into(),
            arguments: diagnostic.clone(),
            response,
            cancellation: tokio_util::sync::CancellationToken::new(),
            transport: None,
        };
        connection
            .dispatch_request(&mut app, request, false)
            .unwrap();
        open_cell_draft(&mut app);
        let deadline = Instant::now() + Duration::from_secs(10);
        while connection.diagnostic.is_some() {
            assert!(Instant::now() < deadline);
            connection.poll_diagnostic(&mut app, false).unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        let result = receive.blocking_recv().unwrap().unwrap();
        assert!(
            result["value"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "out_of_canvas")
        );
        assert_eq!(result["after_revision"], before.0);
    }
    assert_eq!(app.session.ui.table_cell_edit.as_ref().unwrap().text, "9.5");
    assert_eq!(
        (
            app.doc.automation_revision,
            app.doc.edit_generation,
            app.session.undo_stack.len()
        ),
        before
    );

    app.session.ui.phase_edit_active = true;
    for (name, arguments) in [
        ("plotium_render_preview", preview),
        ("plotium_figure_diagnose", diagnostic),
    ] {
        assert_eq!(
            session.call(&mut app, name, arguments).unwrap_err().code,
            "busy"
        );
    }
}

fn pending(
    a: Value,
) -> (
    PendingRequest,
    tokio::sync::oneshot::Receiver<std::result::Result<Value, Failure>>,
) {
    let (response, rx) = tokio::sync::oneshot::channel();
    (
        PendingRequest {
            name: "plotium_plan".into(),
            arguments: a,
            response,
            cancellation: tokio_util::sync::CancellationToken::new(),
            transport: None,
        },
        rx,
    )
}
fn finish(c: &mut Connection, app: &mut PlotiumApp) -> std::result::Result<(), Failure> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while c.transform.is_some() {
        assert!(Instant::now() < deadline);
        c.poll_transform(app, false)?;
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
#[test]
fn plans_are_one_use_permission_bound_and_project_bound_even_at_equal_revision() {
    for tool in [TABLE_PATCH, TABLE_REFRESH] {
        for fault in ["permission", "revision", "project", "busy"] {
            let (mut app, mut s) = fixture();
            let a = args(&app, tool);
            s.allow_modify = false;
            assert_eq!(
                s.call(&mut app, "plotium_plan", a.clone())
                    .unwrap_err()
                    .code,
                "permission_denied"
            );
            s.allow_modify = true;
            let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
            assert_eq!(p["update"]["status"], "prepared");
            let old = app.doc.automation_revision;
            let datasets = app.doc.datasets.clone();
            match fault {
                "permission" => s.allow_modify = false,
                "revision" => app.doc.automation_revision += 1,
                "project" => {
                    app.start_new_project();
                    app.doc.datasets = datasets;
                    app.doc.automation_revision = old;
                }
                "busy" => app.session.ui.project_save_in_progress = true,
                _ => unreachable!(),
            }
            let e = s
                .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
                .unwrap_err();
            assert_eq!(
                e.code,
                match fault {
                    "permission" => "permission_denied",
                    "revision" => "stale_revision",
                    "project" => "project_changed",
                    _ => "busy",
                }
            );
            if fault == "project" {
                assert_eq!(
                    s.call(&mut app, "plotium_plan", a).unwrap_err().code,
                    "project_changed"
                );
            } else {
                s.allow_modify = true;
                app.session.ui.project_save_in_progress = false;
                assert_eq!(
                    s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
                        .unwrap_err()
                        .code,
                    "expired_plan"
                );
            }
        }
    }
}
#[test]
fn background_patch_and_refresh_discard_on_lifecycle_changes_and_disconnect() {
    for tool in [TABLE_PATCH, TABLE_REFRESH] {
        for fault in [
            "permission",
            "revision",
            "table_revision",
            "project",
            "cancel",
            "disconnect",
            "stop",
        ] {
            let (mut app, _) = fixture();
            let a = args(&app, tool);
            let mut c = Connection::start(|| {}).unwrap();
            let (req, mut rx) = pending(a);
            let cancellation = req.cancellation.clone();
            c.dispatch_request(&mut app, req, false).unwrap();
            assert!(c.transform.is_some());
            let original = app.doc.automation_revision;
            match fault {
                "permission" => c.session.allow_modify = false,
                "revision" => app.doc.automation_revision += 1,
                "table_revision" => {
                    app.doc.datasets[usize::from(tool == TABLE_REFRESH)]
                        .as_table_mut()
                        .unwrap()
                        .typed_state
                        .envelope
                        .revision
                        .id = RevisionId::new()
                }
                "project" => {
                    let datasets = app.doc.datasets.clone();
                    app.start_new_project();
                    app.doc.datasets = datasets;
                    app.doc.automation_revision = original;
                }
                "cancel" => cancellation.cancel(),
                "disconnect" => rx.close(),
                "stop" => c.stop(),
                _ => unreachable!(),
            }
            if fault == "stop" {
                assert!(c.transform.is_none());
                continue;
            }
            let expected = match fault {
                "permission" => "permission_denied",
                "revision" | "table_revision" => "stale_revision",
                "project" => "project_changed",
                _ => "request_cancelled",
            };
            if fault == "project" {
                finish(&mut c, &mut app).unwrap();
            } else {
                assert_eq!(finish(&mut c, &mut app).unwrap_err().code, expected);
            }
            if fault != "disconnect" {
                assert_eq!(rx.try_recv().unwrap().unwrap_err().code, expected);
            }
            assert_eq!(app.doc.datasets.len(), 2);
            assert_eq!(
                app.doc.automation_revision,
                original + u64::from(fault == "revision")
            );
        }
    }
}
#[test]
fn exact_values_and_freshness_are_readable_after_patch_refresh_and_undo() {
    let (mut app, mut s) = fixture();
    for tool in [TABLE_PATCH, TABLE_REFRESH] {
        let a = args(&app, tool);
        let p = s.call(&mut app, "plotium_plan", a).unwrap();
        let out = s
            .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap();
        assert!(out.get("produced").is_none());
        let d = app.doc.datasets[1].as_table().unwrap();
        let a = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"dataset_id":d.resource_id,"table_revision":d.typed_state.envelope.revision.id,"include_provenance":true});
        let value = s
            .call(&mut app, "plotium_table_read_rows", a.clone())
            .unwrap();
        assert_eq!(
            value["value"]["freshness"]["status"],
            if tool == TABLE_PATCH {
                "stale"
            } else {
                "current"
            }
        );
        assert_eq!(
            value["value"]["rows"][1]["values"][1]["value"],
            if tool == TABLE_PATCH { 2. } else { 2.5 }
        );
        let meta = s.call(&mut app, "plotium_table_columns", a).unwrap();
        assert!(meta["value"]["provenance"]["refresh_recipe"].is_object());
    }
    app.undo();
    let id = app.doc.datasets[1].resource_id();
    assert_eq!(
        app.doc.table_freshness(id).status,
        TableFreshnessState::Stale
    );
    app.redo();
    assert_eq!(
        app.doc.table_freshness(id).status,
        TableFreshnessState::Current
    );
    let constraint = crate::capabilities::respond(
        serde_json::json!({"section":"operation_constraints","key":TABLE_PATCH}),
    )
    .unwrap();
    assert_eq!(
        constraint["contract"]["schema"]["properties"]["edits"]["maxItems"],
        1000
    );
}
