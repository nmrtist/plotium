use super::*;
#[cfg(unix)]
use plotium_core::project::*;
use plotium_core::state::PlotiumApp;
use serde_json::{Value, json};
fn project() -> PlotiumApp {
    let mut a = PlotiumApp::new_with_settings(Default::default());
    a.start_new_project();
    a.mark_document_dirty();
    a
}
fn plan(s: &mut Session, a: &mut PlotiumApp, path: Option<&std::path::Path>) -> Value {
    s.call(a,"plotium_plan",json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"tool_id":"project.save","parameters":path.map(|p|json!({"path":p})).unwrap_or(json!({}))})).unwrap()
}
fn submit(s: &mut Session, a: &mut PlotiumApp, p: &Value) -> String {
    let result = s
        .call(a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(result["save"]["status"], "awaiting_authorization");
    p["save_id"].as_str().unwrap().into()
}
#[cfg(unix)]
#[test]
fn save_requires_distinct_desktop_permission_and_overwrite_confirmation() {
    isolated(
        "save_tests::save_requires_distinct_desktop_permission_and_overwrite_confirmation",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("save.plotium");
            let mut a = project();
            let mut s = Session::default();
            let p = plan(&mut s, &mut a, Some(&path));
            let id = submit(&mut s, &mut a, &p);
            assert!(!path.exists());
            assert!(!s.allow_modify);
            a.decide_external_save(&id, false, false);
            assert_eq!(
                s.call(&mut a, "plotium_save_status", json!({"save_id":id}))
                    .unwrap()["save"]["phase"],
                "not_installed"
            );
            assert!(!path.exists());
            std::fs::write(&path, b"sacrificial old file").unwrap();
            let p = plan(&mut s, &mut a, Some(&path));
            let id = submit(&mut s, &mut a, &p);
            a.decide_external_save(&id, true, false);
            assert!(a.capture_external_save(&id).is_none());
            assert_eq!(std::fs::read(&path).unwrap(), b"sacrificial old file");
            let p = plan(&mut s, &mut a, Some(&path));
            let id = submit(&mut s, &mut a, &p);
            a.decide_external_save(&id, true, true);
            let request = a.capture_external_save(&id).unwrap();
            let snapshot = request.identity().clone();
            let revision = request.persisted_revision().to_owned();
            let outcome = save_project_snapshot(request);
            assert!(outcome.is_ok());
            let operation = a.session.begin_operation();
            assert!(a.complete_captured_project_save(
                operation, &path, false, &snapshot, &revision, outcome
            ));
            assert!(!a.doc.dirty);
            assert!(!a.can_undo()); // Saving never creates an Undo step for disk.
            assert!(load_project(&path).is_ok());
            a.finish_external_save(&id, SavePhase::Committed, true, None);
            let before = a.doc.edit_generation;
            s.call(&mut a, "plotium_save_status", json!({"save_id":id}))
                .unwrap();
            assert_eq!(before, a.doc.edit_generation);
            a.session.ui.project_save_in_progress = false;
            s.call(&mut a, "plotium_get_state", json!({})).unwrap();
            assert!(!a.doc.dirty);
            assert_eq!(
                plan(&mut s, &mut a, None)["preflight"]["path"],
                json!(path.canonicalize().unwrap())
            );
        },
    );
}
#[cfg(unix)]
#[test]
fn captured_worker_isolated_from_new_edits_same_revision_projects_and_connection_stop() {
    isolated(
        "save_tests::captured_worker_isolated_from_new_edits_same_revision_projects_and_connection_stop",
        || {
            for mode in ["edit", "project", "stop"] {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("save.plotium");
                let mut a = project();
                a.doc
                    .canvases
                    .push(plotium_core::state::CanvasDocument::new(
                        "captured before edit".into(),
                        [100., 80.],
                    ));
                let mut c = Connection::start(|| {}).unwrap();
                let p = plan(&mut c.session, &mut a, Some(&path));
                let id = submit(&mut c.session, &mut a, &p);
                a.decide_external_save(&id, true, false);
                let request = a.capture_external_save(&id).unwrap();
                let identity = request.identity().clone();
                let persisted = request.persisted_revision().to_owned();
                let (tx, rx) = std::sync::mpsc::channel();
                let worker = std::thread::spawn(move || {
                    rx.recv().unwrap();
                    save_project_snapshot(request)
                });
                match mode {
                    "edit" => {
                        a.doc.canvases[0].name = "new unsaved edit".into();
                        a.mark_document_dirty();
                    }
                    "project" => a.start_new_project(),
                    _ => c.stop(),
                };
                tx.send(()).unwrap();
                let result = worker.join().unwrap();
                if mode == "edit" {
                    assert!(result.is_ok());
                    let operation = a.session.begin_operation();
                    assert!(a.complete_captured_project_save(
                        operation, &path, false, &identity, &persisted, result
                    ));
                    assert!(a.doc.dirty);
                    assert_eq!(
                        load_project(&path).unwrap().doc.canvases[0].name,
                        "captured before edit"
                    );
                    assert_eq!(a.doc.canvases[0].name, "new unsaved edit");
                } else {
                    assert_eq!(
                        result.as_ref().unwrap_err().save_phase(),
                        SavePhase::NotInstalled
                    );
                    assert!(!path.exists());
                    assert!(!a.external_save_can_apply(&id));
                    if mode == "project" {
                        assert_eq!(a.doc.automation_revision, identity.document_revision);
                        assert!(a.doc.project_path.is_none());
                        assert!(!a.doc.dirty);
                    }
                }
            }
        },
    );
}
#[test]
fn target_change_and_cancelled_confirmation_never_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("target.plotium");
    let mut a = project();
    let mut s = Session::default();
    let p = plan(&mut s, &mut a, Some(&path));
    let id = submit(&mut s, &mut a, &p);
    std::fs::write(&path, b"created after plan").unwrap();
    a.decide_external_save(&id, true, true);
    assert!(a.capture_external_save(&id).is_none());
    assert_eq!(std::fs::read(&path).unwrap(), b"created after plan");
    let p = plan(&mut s, &mut a, Some(&path));
    let id = submit(&mut s, &mut a, &p);
    drop(s);
    a.session.external_saves.reconcile();
    a.decide_external_save(&id, true, true);
    assert!(a.capture_external_save(&id).is_none());
    assert_eq!(std::fs::read(&path).unwrap(), b"created after plan");
}

// HOME is process-global: never mutate it in the parallel test harness. The
// native completion path persists settings, so run these Unix filesystem tests
// in an independently launched harness with its own verified platform paths.
#[cfg(unix)]
fn isolated(name: &str, run: impl FnOnce()) {
    const KEY: &str = "PLOTIUM_SAVE_TEST_HOME";
    if let Some(home) = std::env::var_os(KEY) {
        let home = std::path::PathBuf::from(home);
        let config = plotium_core::settings::config_dir().unwrap();
        let data = plotium_core::settings::data_local_dir().unwrap();
        assert!(
            config.starts_with(&home),
            "Unexpected settings path: {config:?}"
        );
        assert!(data.starts_with(&home), "Unexpected data path: {data:?}");
        eprintln!("Isolated native save test: config={config:?}; data={data:?}");
        run();
    } else {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path().join(".config"))
            .env("XDG_DATA_HOME", home.path().join(".local/share"))
            .env(KEY, home.path())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprint!("{stderr}");
        assert!(
            stderr.contains("Isolated native save test:"),
            "Child test body was not entered: {name}"
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
