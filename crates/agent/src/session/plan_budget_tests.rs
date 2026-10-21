//! Settings-free fixtures; all file input/targets are confined to one tempfile dir.
use super::*;
use plotium_core::{
    data::*,
    state::{CanvasDocument, Dataset, FloatSeries, materialized_float_series_table},
};
use std::{path::Path, sync::atomic::AtomicBool};
const TABLE_TOOLS: [&str; 4] = ["data.import", "data.transform", TABLE_PATCH, TABLE_REFRESH];
fn fixture(dir: &Path) -> (PlotiumApp, Session) {
    std::fs::write(dir.join("input.csv"), "x,y\n0,1\n1,2\n").unwrap();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc
        .canvases
        .push(CanvasDocument::new("budget".into(), [180., 90.]));
    let table = materialized_float_series_table(
        ("x".into(), "".into(), vec![Some(0.), Some(1.)]),
        vec![FloatSeries {
            name: "y".into(),
            unit: "".into(),
            values: vec![Some(1.), Some(2.)],
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.r11.plan_budget",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut session = Session::default();
    session.allow_modify = true;
    let a = args(&app, "data.transform", dir);
    let p = session.call(&mut app, "plotium_plan", a).unwrap();
    session
        .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    (app, session)
}
fn args(app: &PlotiumApp, tool: &str, dir: &Path) -> Value {
    let mut a = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"tool_id":tool});
    let canvas = app.doc.canvases[0].resource_id;
    a["parameters"] = match tool {
        "figure.export" => {
            json!({"canvas":TargetRef::resource(canvas.into()),"outputs":[{"format":"svg","path":dir.join("figure.svg")}]})
        }
        "project.save" => json!({"path":dir.join("project.plotium")}),
        "resource.rename" => {
            a["ids"] = json!([canvas]);
            json!({"name":"budget renamed"})
        }
        FIGURE_EDIT => {
            json!({"canvas":TargetRef::resource(canvas.into()),"edits":[{"op":"page","size_mm":[180,90]}]})
        }
        "data.import" => {
            json!({"mode":"table_data_only","paths":[dir.join("input.csv")],"header":"present"})
        }
        _ => {
            let table = app.doc.datasets[usize::from(tool == TABLE_REFRESH)]
                .as_table()
                .unwrap();
            let revision = &table.typed_state.envelope.revision;
            a["ids"] = json!([table.resource_id]);
            match tool {
                "data.transform" => {
                    json!({"mode":"table_data_only","name":"Derived","memory_limit_bytes":16*1024*1024,"plan":RelPlanV1::new(Relation::SnapshotRead(SnapshotRead{table:revision.table_id,revision:revision.id,fingerprint:revision.snapshot.fingerprint}))})
                }
                TABLE_PATCH => {
                    let column = table.series_bindings[0].value_column;
                    let row = table.typed_rows(2, &[column]).unwrap().row_ids[1];
                    json!({"table_revision":revision.id,"edits":[{"row_id":row,"column_id":column,"value":{"kind":"float64","value":2.5}}]})
                }
                TABLE_REFRESH => json!({"table_revision":revision.id}),
                _ => unreachable!(),
            }
        }
    };
    a
}
fn retain(s: &mut Session, app: &mut PlotiumApp, dir: &Path, tool: &str) -> Value {
    let a = args(app, tool, dir);
    s.call(app, "plotium_plan", a).unwrap()
}
fn fill_exports(s: &mut Session, app: &mut PlotiumApp, dir: &Path, n: usize) {
    for _ in 0..n {
        retain(s, app, dir, "figure.export");
    }
}
#[test]
fn full_export_queue_blocks_every_table_preparation_entry() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, mut s) = fixture(dir.path());
    fill_exports(&mut s, &mut app, dir.path(), 32);
    let errors = TABLE_TOOLS.map(|tool| {
        let a = args(&app, tool, dir.path());
        s.call(&mut app, "plotium_plan", a).err().map(|e| e.code)
    });
    assert_eq!(errors, [Some("plan_limit"); 4]);
}
#[test]
fn export_queue_is_rechecked_when_background_table_preparation_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let errors = TABLE_TOOLS.map(|tool| {
        let (mut app, mut s) = fixture(dir.path());
        let a = args(&app, tool, dir.path());
        if tool == "data.import" {
            let (plan, params) = s.prepare_import_plan(&app, a, false).unwrap();
            let prepared = PreparedDataImport::prepare(params, || false).unwrap();
            fill_exports(&mut s, &mut app, dir.path(), 32);
            s.finish_import_plan(&app, plan, prepared, false)
                .err()
                .map(|e| e.code)
        } else {
            let (plan, snapshot) = s.prepare_transform_plan(&app, a, false).unwrap();
            let prepared = snapshot.prepare(&AtomicBool::new(false)).unwrap();
            fill_exports(&mut s, &mut app, dir.path(), 32);
            s.finish_transform_plan(&app, plan, prepared, false)
                .err()
                .map(|e| e.code)
        }
    });
    assert_eq!(errors, [Some("plan_limit"); 4]);
}
#[test]
fn mixed_queue_counts_table_payloads_once_and_releases_only_one_slot() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, mut s) = fixture(dir.path());
    fill_exports(&mut s, &mut app, dir.path(), 20);
    for _ in 0..4 {
        retain(&mut s, &mut app, dir.path(), "project.save");
    }
    for _ in 0..4 {
        retain(&mut s, &mut app, dir.path(), "resource.rename");
    }
    for tool in TABLE_TOOLS {
        retain(&mut s, &mut app, dir.path(), tool);
    }
    assert_eq!(
        (
            s.plans.len(),
            s.saves.len(),
            s.exports.len(),
            s.imports.len(),
            s.transforms.len()
        ),
        (8, 4, 20, 1, 3)
    );
    for tool in [
        "figure.export",
        "project.save",
        "resource.rename",
        FIGURE_EDIT,
        "data.import",
        "data.transform",
        TABLE_PATCH,
        TABLE_REFRESH,
    ] {
        let a = args(&app, tool, dir.path());
        assert_eq!(
            s.call(&mut app, "plotium_plan", a).unwrap_err().code,
            "plan_limit",
            "{tool}"
        );
    }
    let id = s.exports.keys().next().unwrap().clone();
    assert_eq!(
        s.call_when_available(&mut app, "plotium_execute", json!({"plan_id":id}), true)
            .unwrap_err()
            .code,
        "busy"
    );
    assert_eq!(s.exports.len(), 19);
    retain(&mut s, &mut app, dir.path(), "data.import");
    assert_eq!(
        (
            s.plans.len(),
            s.saves.len(),
            s.exports.len(),
            s.imports.len(),
            s.transforms.len()
        ),
        (9, 4, 19, 2, 3)
    );
    for tool in TABLE_TOOLS {
        let a = args(&app, tool, dir.path());
        assert_eq!(
            s.call(&mut app, "plotium_plan", a).unwrap_err().code,
            "plan_limit"
        );
    }
    assert!(app.session.external_exports.pending().is_none());
    assert!(!dir.path().join("figure.svg").exists());
    assert!(!dir.path().join("project.plotium").exists());
}
#[test]
fn import_candidate_limit_remains_four_below_the_shared_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, mut s) = fixture(dir.path());
    for _ in 0..4 {
        retain(&mut s, &mut app, dir.path(), "data.import");
    }
    let a = args(&app, "data.import", dir.path());
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "plan_limit"
    );
    assert_eq!((s.plans.len(), s.imports.len()), (4, 4));
}

fn completed_exports_release_connection_permits(outcome: &str) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, mut s) = fixture(dir.path());
    let mut ids = Vec::new();
    for index in 0..24 {
        let path = if outcome == "invalid" {
            dir.path().join("missing/figure.svg")
        } else {
            dir.path().join(format!("figure-{index}.svg"))
        };
        let mut a = args(&app, "figure.export", dir.path());
        a["parameters"]["outputs"][0]["path"] = json!(path);
        let plan = s.call(&mut app, "plotium_plan", a).unwrap();
        let id = plan["export_id"].as_str().unwrap().to_owned();
        s.call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
        if outcome != "invalid" {
            // Completed permits must be released, but pending consent must retain
            // the live permit so Stop/access/context can still cancel it.
            assert!(s.export_permits.last().is_some_and(|p| !p.is_revoked()));
            if outcome == "failed" {
                std::fs::write(&path, b"intervening external file").unwrap();
            }
            app.decide_external_export(&id, true, false);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while app.poll_external_export() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let receipt = app.session.external_exports.receipt(&id).unwrap();
        assert_eq!(receipt.status, plotium_core::state::ExportStatus::Completed);
        assert_eq!(
            receipt.files[0].status,
            if outcome == "succeeded" {
                "succeeded"
            } else {
                "failed"
            }
        );
        if outcome == "failed" {
            assert_eq!(std::fs::read(path).unwrap(), b"intervening external file");
        }
        ids.push(id);
    }
    // The process keeps 16 receipts, independent of connection permit cleanup.
    let generation = app.doc.edit_generation;
    assert_eq!(
        s.call(
            &mut app,
            "plotium_export_status",
            json!({"export_id":ids[7]})
        )
        .unwrap_err()
        .code,
        "unknown_export"
    );
    for id in &ids[8..] {
        let status = s
            .call(&mut app, "plotium_export_status", json!({"export_id":id}))
            .unwrap();
        assert_eq!(status["export"]["status"], "completed");
    }
    assert_eq!(app.doc.edit_generation, generation);
    assert!(
        s.export_permits.len() <= 1,
        "{outcome}: {} completed permits accumulated",
        s.export_permits.len()
    );
    let next = retain(&mut s, &mut app, dir.path(), "figure.export");
    s.call(
        &mut app,
        "plotium_execute",
        json!({"plan_id":next["plan_id"]}),
    )
    .unwrap();
    assert_eq!(s.export_permits.len(), 1);
    assert!(!s.export_permits[0].is_revoked());
    s.set_allow_modify(false);
    assert!(s.export_permits.is_empty());
    assert_eq!(
        s.call(
            &mut app,
            "plotium_export_status",
            json!({"export_id":next["export_id"]})
        )
        .unwrap()["export"]["files"][0]["error_code"],
        "request_cancelled"
    );
}

#[test]
fn completed_invalid_exports_keep_connection_permits_bounded() {
    completed_exports_release_connection_permits("invalid");
}
#[test]
fn completed_successful_exports_keep_connection_permits_bounded() {
    completed_exports_release_connection_permits("succeeded");
}
#[test]
fn completed_failed_exports_keep_connection_permits_bounded() {
    completed_exports_release_connection_permits("failed");
}
