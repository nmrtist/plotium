//! Export fixtures use only tempfile outputs and a settings-free in-memory app.
use super::*;
use plotium_core::{
    automation::TargetRef,
    state::{
        AssetId, CanvasDocument, CanvasObject, CanvasObjectKind, ObjectFrame, ObjectId, PlotiumApp,
        RasterImageContent,
    },
};
use serde_json::{Value, json};
fn project() -> PlotiumApp {
    let mut a = PlotiumApp::new_with_settings(Default::default());
    a.doc
        .canvases
        .push(CanvasDocument::new("export".into(), [180.0, 90.0]));
    a
}
fn plan(s: &mut Session, a: &mut PlotiumApp, outputs: Value) -> Value {
    let c = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    s.call(a,"plotium_plan",json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"tool_id":"figure.export","parameters":{"canvas":c,"outputs":outputs,"size_mm":[180,90],"dpi":300}})).unwrap()
}
fn submit(s: &mut Session, a: &mut PlotiumApp, p: &Value) -> String {
    s.call(a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    p["export_id"].as_str().unwrap().into()
}
fn finish(a: &mut PlotiumApp) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while a.poll_external_export() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn export_readonly_needs_desktop_consent_and_reports_every_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = project();
    let mut s = Session::default();
    let png = dir.path().join("figure.png");
    let svg = dir.path().join("figure.svg");
    let pdf = dir.path().join("figure.pdf");
    let bad = dir.path().join("missing/bad.svg");
    let p = plan(
        &mut s,
        &mut a,
        json!([{"format":"png","path":png},{"format":"svg","path":svg},{"format":"pdf","path":pdf},{"format":"svg","path":bad}]),
    );
    assert_eq!(p["preflight"]["png_dimensions"], json!([2126, 1063]));
    let id = submit(&mut s, &mut a, &p);
    assert!(!png.exists());
    assert!(!s.allow_modify);
    assert_eq!(
        a.session.external_exports.receipt(&id).unwrap().files[3].status,
        "failed"
    );
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    let r = a.session.external_exports.receipt(&id).unwrap();
    assert_eq!(
        r.files
            .iter()
            .map(|f| f.status.as_str())
            .collect::<Vec<_>>(),
        ["succeeded", "succeeded", "succeeded", "failed"]
    );
    let png = std::fs::read(png).unwrap();
    assert_eq!(&png[16..24], &[0, 0, 8, 78, 0, 0, 4, 39]);
    assert!(std::fs::read(svg).unwrap().starts_with(b"<svg"));
    assert!(std::fs::read(pdf).unwrap().starts_with(b"%PDF"));
    assert!(!bad.exists());
    assert!(a.session.status.contains("Output directory unavailable"));
    assert!(!a.can_undo());
    let generation = a.doc.edit_generation;
    s.call(&mut a, "plotium_export_status", json!({"export_id":id}))
        .unwrap();
    assert_eq!(a.doc.edit_generation, generation);
    assert_eq!(
        s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "expired_plan"
    );
}
#[test]
fn export_overwrite_requires_request_and_unchecked_desktop_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.svg");
    std::fs::write(&path, b"sacrificial original").unwrap();
    let mut a = project();
    let mut s = Session::default();
    let p = plan(&mut s, &mut a, json!([{"format":"svg","path":path}]));
    let id = submit(&mut s, &mut a, &p);
    assert_eq!(
        a.session.external_exports.receipt(&id).unwrap().files[0]
            .error_code
            .as_deref(),
        Some("overwrite_not_requested")
    );
    assert!(a.session.external_exports.pending().is_none());
    let p = plan(
        &mut s,
        &mut a,
        json!([{"format":"svg","path":path,"overwrite":true}]),
    );
    let id = submit(&mut s, &mut a, &p);
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    assert_eq!(std::fs::read(&path).unwrap(), b"sacrificial original");
    let p = plan(
        &mut s,
        &mut a,
        json!([{"format":"svg","path":path,"overwrite":true}]),
    );
    let id = submit(&mut s, &mut a, &p);
    a.decide_external_export(&id, true, true);
    finish(&mut a);
    assert!(std::fs::read(path).unwrap().starts_with(b"<svg"));
}
#[test]
fn export_context_permission_revision_revocation_and_missing_assets_do_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("revoked.svg");
    let mut a = project();
    let mut s = Session::default();
    let p = plan(&mut s, &mut a, json!([{"format":"svg","path":path}]));
    let id = submit(&mut s, &mut a, &p);
    s.set_allow_modify(true);
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    assert!(!path.exists());
    let p = plan(&mut s, &mut a, json!([{"format":"svg","path":path}]));
    let id = submit(&mut s, &mut a, &p);
    a.doc.automation_revision += 1;
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    assert!(!path.exists());
    let p = plan(&mut s, &mut a, json!([{"format":"svg","path":path}]));
    let id = submit(&mut s, &mut a, &p);
    a.session.project_context = Default::default();
    s.synchronize_context(&a);
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    assert!(!path.exists());
    let status = s
        .call(&mut a, "plotium_export_status", json!({"export_id":id}))
        .unwrap();
    assert_eq!(status["matches_current_project"], false);
    a.doc.canvases[0].objects.push(CanvasObject {
        id: ObjectId::new(1),
        name: "missing".into(),
        frame: ObjectFrame::new(0.0, 0.0, 10.0, 10.0),
        locked: false,
        visible: true,
        kind: CanvasObjectKind::RasterImage(RasterImageContent::new(AssetId::new())),
    });
    let p = plan(&mut s, &mut a, json!([{"format":"svg","path":path}]));
    let id = submit(&mut s, &mut a, &p);
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    let f = &a.session.external_exports.receipt(&id).unwrap().files[0];
    assert_eq!(f.status, "failed");
    assert!(f.message.contains("missing"));
    assert!(!path.exists());
}
#[test]
fn export_target_drift_is_rejected_without_truncating_existing_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("changed.svg");
    let mut a = project();
    let mut s = Session::default();
    let p = plan(&mut s, &mut a, json!([{"format":"svg","path":path}]));
    let id = submit(&mut s, &mut a, &p);
    std::fs::write(&path, b"new external file").unwrap();
    a.decide_external_export(&id, true, false);
    finish(&mut a);
    assert_eq!(std::fs::read(path).unwrap(), b"new external file");
    assert_eq!(
        a.session.external_exports.receipt(&id).unwrap().files[0].status,
        "failed"
    );
}
