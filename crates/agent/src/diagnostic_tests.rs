use super::*;
use crate::Session;
use plotium_core::project::save_project;
use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
fn fixture() -> (PlotiumApp, Session, Vec<TargetRef>) {
    let (mut a, mut s, ids) = crate::guide_fixture::fixture();
    crate::guide_fixture::write(
        &mut a,
        &mut s,
        &ids[..1],
        plotium_core::properties::guide::POSITION.as_str(),
        json!([0.4, 0.5]),
    );
    let c = &mut a.doc.canvases[0];
    c.name = "R15 diagnostics — synthetic".into();
    let w = c.size_pt()[0];
    let mut targets = vec![];
    for (text, frame) in [
        ("OUT", [w - 8., 200., 35., 20.]),
        ("Long text 测试 without wrapping", [20., 235., 20., 10.]),
        ("Intentional A", [280., 235., 90., 18.]),
        ("Intentional B", [310., 235., 90., 18.]),
    ] {
        let id = c.allocate_object_id();
        c.objects.push(ContentItem {
            id,
            name: "Same text".into(),
            frame: ObjectFrame::new(frame[0], frame[1], frame[2], frame[3]),
            locked: false,
            visible: true,
            kind: ContentKind::Text(TextBox::label(text.into())),
        });
        targets.push(TargetRef::resource(canvas_object_ref(c.resource_id, id)));
    }
    a.clear_history();
    s.allow_modify = false;
    (a, s, targets)
}
fn args(a: &PlotiumApp, targets: &[TargetRef]) -> Value {
    json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"canvas":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"output":{"size_mm":a.doc.canvases[0].size_mm},"overlap_targets":&targets[2..],"limit":200})
}
fn all(a: &mut PlotiumApp, s: &mut Session, mut q: Value) -> (Vec<Value>, Value) {
    let mut rows = vec![];
    for _ in 0..1000 {
        let page = s.call(a, "plotium_figure_diagnose", q).unwrap()["value"].clone();
        rows.extend(page["items"].as_array().unwrap().iter().cloned());
        if page["continuation"].is_null() {
            assert_eq!(page["coverage"]["scan_complete"], true);
            return (rows, page);
        }
        q = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"cursor":page["continuation"]});
    }
    panic!("unbounded scan")
}
fn issue(rows: &[Value], code: &str, t: &TargetRef) -> bool {
    rows.iter()
        .any(|r| r["kind"] == "issue" && r["code"] == code && r["target"] == json!(t))
}
#[test]
fn diagnostic_fixture_localizes_reads_and_preserves_native_state() {
    let (mut a, mut s, t) = fixture();
    let q = args(&a, &t);
    let frames: Vec<_> = a.doc.canvases[0]
        .objects
        .iter()
        .map(|o| (o.id, o.frame, o.visible))
        .collect();
    let plots: Vec<_> = a.doc.canvases[0]
        .objects
        .iter()
        .filter_map(|o| o.plot())
        .map(|p| {
            (
                p.binding.clone(),
                p.axis_overrides.clone(),
                serde_json::to_value(p.figure()).unwrap(),
            )
        })
        .collect();
    let rev = a.doc.automation_revision;
    let undo = a.session.undo_stack.len();
    a.doc.dirty = false;
    let generation = a.doc.edit_generation;
    let runs = a.doc.automation_runs.len();
    let (rows, last) = all(&mut a, &mut s, q);
    assert!(issue(&rows, "out_of_canvas", &t[0]));
    assert!(issue(&rows, "text_container_overflow", &t[1]));
    assert!(issue(&rows, "content_frame_overlap", &t[2]));
    assert!(
        rows.iter()
            .any(|r| r["code"] == "legend_occlusion_potential")
    );
    assert!(
        rows.iter()
            .filter(|r| r["code"] == "legend_occlusion_potential")
            .all(|r| r["target"]["component"]["kind"] == "series"
                && r["evidence"]["other_component"] == "guide.legend")
    );
    assert!(
        rows.iter()
            .any(|r| r["measurement_basis"] == "text_estimate")
    );
    assert_eq!(last["output"]["coordinate_space"], "output_page_pt");
    assert_eq!(last["coverage"]["sampling"], false);
    assert_eq!(a.doc.automation_revision, rev);
    assert_eq!(a.session.undo_stack.len(), undo);
    assert!(!a.doc.dirty);
    assert_eq!(a.doc.edit_generation, generation);
    assert_eq!(a.doc.automation_runs.len(), runs);
    assert_eq!(
        a.doc.canvases[0]
            .objects
            .iter()
            .map(|o| (o.id, o.frame, o.visible))
            .collect::<Vec<_>>(),
        frames
    );
    assert_eq!(
        a.doc.canvases[0]
            .objects
            .iter()
            .filter_map(|o| o.plot())
            .map(|p| (
                p.binding.clone(),
                p.axis_overrides.clone(),
                serde_json::to_value(p.figure()).unwrap()
            ))
            .collect::<Vec<_>>(),
        plots
    );
    for r in rows.iter().filter(|r| r["kind"] == "issue") {
        let q = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"target":r["target"],"view":"self"});
        assert!(s.call(&mut a, "plotium_document_inspect", q).is_ok());
    }
}
#[test]
fn physical_scale_recomputes_and_native_edit_changes_only_corresponding_finding() {
    let (mut a, mut s, t) = fixture();
    let q = args(&a, &t);
    let (before, _) = all(&mut a, &mut s, q.clone());
    let mut scaled = q.clone();
    scaled["output"]["size_mm"] = json!([90., 45.]);
    let (small, _) = all(&mut a, &mut s, scaled);
    let bound = |rows: &[Value]| {
        rows.iter()
            .find(|r| {
                r["target"] == json!(t[0])
                    && r["kind"] == "layout"
                    && r["component"] == "content.frame"
            })
            .unwrap()["evidence"]["bounds"]["x"]
            .as_f64()
            .unwrap()
    };
    assert!((bound(&small) * 2. - bound(&before)).abs() < 0.001);
    s.allow_modify = true;
    let p=s.call(&mut a,"plotium_plan",json!({"expected_project_context":q["expected_project_context"],"expected_revision":q["expected_revision"],"tool_id":"figure.edit","parameters":{"canvas":q["canvas"],"edits":[{"op":"content_frame","target":t[0],"frame_pt":[440.,200.,35.,20.]},{"op":"content_frame","target":t[1],"frame_pt":[20.,235.,300.,18.]}]}})).unwrap();
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let q = args(&a, &t);
    let (after, _) = all(&mut a, &mut s, q);
    assert!(!issue(&after, "out_of_canvas", &t[0]));
    assert!(!issue(&after, "text_container_overflow", &t[1]));
    assert!(issue(&after, "content_frame_overlap", &t[2]));
}
#[test]
fn paging_binds_context_revision_configuration_and_one_use_cursor() {
    let (mut a, mut s, t) = fixture();
    let mut q = args(&a, &t);
    q["limit"] = json!(1);
    let first = s
        .call(&mut a, "plotium_figure_diagnose", q.clone())
        .unwrap()["value"]
        .clone();
    assert_eq!(first["truncated"], true);
    let continuation = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"cursor":first["continuation"]});
    let mut bad = continuation.clone();
    bad["output"] = json!({"size_mm":[90.,45.]});
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", bad)
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
    let mut wrong_revision = continuation.clone();
    wrong_revision["expected_revision"] = json!(a.doc.automation_revision + 1);
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", wrong_revision)
            .unwrap_err()
            .code,
        "stale_revision"
    );
    // An unrelated stale request must not consume a still-current cursor.
    let next = s
        .call(&mut a, "plotium_figure_diagnose", continuation.clone())
        .unwrap();
    assert_eq!(next["value"]["output"], first["output"]);
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", continuation)
            .unwrap_err()
            .code,
        "expired_cursor"
    );
    let stale = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"cursor":next["value"]["continuation"]});
    a.doc.automation_revision += 1;
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", stale)
            .unwrap_err()
            .code,
        "stale_revision"
    );
    a.session.project_context = ProjectContextId::default();
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", q)
            .unwrap_err()
            .code,
        "project_changed"
    );
}
fn pending(q: Value) -> (PendingRequest, oneshot::Receiver<Result<Value, Failure>>) {
    let (response, rx) = oneshot::channel();
    (
        PendingRequest {
            name: "plotium_figure_diagnose".into(),
            arguments: q,
            response,
            cancellation: CancellationToken::new(),
            transport: None,
        },
        rx,
    )
}
fn wait(c: &mut Connection, a: &mut PlotiumApp) {
    let end = Instant::now() + Duration::from_secs(15);
    while c.diagnostic.is_some() && Instant::now() < end {
        let _ = c.poll_diagnostic(a, false);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(c.diagnostic.is_none());
}
#[test]
fn background_completion_rechecks_revision_cancel_project_and_connection() {
    for mode in [
        "read_only",
        "revision",
        "cancel",
        "project",
        "stop",
        "disconnect",
    ] {
        let (mut a, _, t) = fixture();
        let mut c = Connection::start(|| {}).unwrap();
        c.session.allow_modify = false;
        let (req, rx) = pending(args(&a, &t));
        let cancel = req.cancellation.clone();
        c.dispatch_request(&mut a, req, false).unwrap();
        match mode {
            "revision" => a.doc.automation_revision += 1,
            "cancel" => cancel.cancel(),
            "project" => a.session.project_context = ProjectContextId::default(),
            "stop" => c.stop(),
            _ => {}
        }
        if mode == "disconnect" {
            drop(rx);
            wait(&mut c, &mut a);
            assert!(c.session.diagnostic.is_none());
            continue;
        }
        wait(&mut c, &mut a);
        let result = rx.blocking_recv().unwrap();
        if mode == "read_only" {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result.unwrap_err().code,
                match mode {
                    "revision" => "stale_revision",
                    "project" => "project_changed",
                    "stop" => "connection_stopped",
                    _ => "request_cancelled",
                }
            );
        }
        assert!(c.session.diagnostic.is_none());
    }
}
#[test]
fn discovery_and_budget_errors_are_explicit() {
    let schema = crate::protocol::definitions()
        .into_iter()
        .find(|t| t.name == "plotium_figure_diagnose")
        .unwrap();
    let schema = Value::Object((*schema.input_schema).clone());
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("expected_project_context"))
    );
    assert_eq!(schema["oneOf"].as_array().unwrap().len(), 2);
    let (mut a, mut s, t) = fixture();
    let mut q = args(&a, &t);
    q["output"]["size_mm"] = json!([90, 90]);
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", q)
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
    a.doc.canvases[0].objects[3].text_mut().unwrap().text = "X".repeat(DIAGNOSTIC_TEXT_BYTES + 1);
    let q = args(&a, &t);
    assert_eq!(
        s.call(&mut a, "plotium_figure_diagnose", q)
            .unwrap_err()
            .code,
        "diagnostic_budget_exceeded"
    );
}
#[test]
#[ignore = "explicit synthetic fixture output directory required"]
fn write_diagnostic_acceptance_fixture() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("PLOTIUM_DIAGNOSTIC_FIXTURE").expect("set output directory"),
    );
    assert!(dir.is_absolute() && !dir.exists());
    std::fs::create_dir_all(&dir).unwrap();
    let (mut a, mut s, t) = fixture();
    let q = args(&a, &t);
    let (rows, last) = all(&mut a, &mut s, q.clone());
    std::fs::write(
        dir.join("diagnostics.json"),
        serde_json::to_vec_pretty(&json!({"request":q,"rows":rows,"last":last})).unwrap(),
    )
    .unwrap();
    save_project(&a, &dir.join("r15.plotium"), false).unwrap();
    let preview = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"target":q["canvas"],"max_edge":1440});
    let result = s.call(&mut a, "plotium_render_preview", preview).unwrap();
    std::fs::write(
        dir.join("native-preview.json"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
}

#[test]
fn hidden_native_series_produces_no_potential_occlusion() {
    let (mut a, mut s, t) = fixture();
    let q = args(&a, &t);
    let (before, _) = all(&mut a, &mut s, q.clone());
    let target = before
        .iter()
        .find(|r| r["code"] == "legend_occlusion_potential")
        .unwrap()["target"]
        .clone();
    s.allow_modify = true;
    let p=s.call(&mut a,"plotium_plan",json!({"expected_project_context":q["expected_project_context"],"expected_revision":q["expected_revision"],"tool_id":"properties.set","targets":[target],"parameters":{"key":"series.visible","value":false}})).unwrap();
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let q = args(&a, &t);
    let (after, _) = all(&mut a, &mut s, q);
    assert!(
        !after
            .iter()
            .any(|r| r["target"] == target && r["code"] == "legend_occlusion_potential")
    );
    assert!(
        after
            .iter()
            .any(|r| r["target"] == target && r["reason"] == "hidden_series")
    );
}

#[test]
fn saved_views_restore_native_curve_targets_without_persisting_diagnostics() {
    let (a, _, t) = fixture();
    let dir = tempfile::tempdir().unwrap();
    for snapshots in [false, true] {
        let path = dir.path().join(format!("fixture-{snapshots}.plotium"));
        save_project(&a, &path, snapshots).unwrap();
        let mut reopened = plotium_core::project::load_project(&path).unwrap();
        let mut session = Session::default();
        let q = args(&reopened, &t);
        let (rows, _) = all(&mut reopened, &mut session, q);
        assert!(
            rows.iter()
                .any(|r| r["code"] == "legend_occlusion_potential")
        );
        assert!(!rows.iter().any(|r| {
            r["reason"]
                .as_str()
                .is_some_and(|s| s.contains("no verified native provenance"))
        }));
    }
}
