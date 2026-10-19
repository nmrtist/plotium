use super::*;
use plotium_core::{DiffusionModel, state::PlotiumApp};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr")
        .join(name)
        .canonicalize()
        .unwrap()
}
fn args(app: &PlotiumApp, paths: &[PathBuf]) -> Value {
    json!({"tool_id":"data.import","parameters":{"paths":paths},"expected_revision":app.doc.automation_revision,"expected_project_context":app.session.project_context})
}
fn plan(s: &mut Session, app: &mut PlotiumApp, paths: &[PathBuf]) -> Value {
    let a = args(app, paths);
    s.call(app, "plotium_plan", a).unwrap()
}
fn execute(s: &mut Session, app: &mut PlotiumApp, p: &Value) -> Result<Value, Failure> {
    s.call(app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
}
fn session() -> Session {
    let mut s = Session::default();
    s.allow_modify = true;
    s
}

#[test]
fn welcome_state_is_readable_without_creating_or_auditing_a_project() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut s = session();
    let context = app.session.project_context;
    for blocked in [false, true] {
        let v = s
            .call_when_available(&mut app, "plotium_get_state", json!({}), blocked)
            .unwrap();
        assert_eq!(v["project_present"], false);
        assert!(v["project"].is_null());
        assert_eq!(v["availability"]["native_import"]["available"], !blocked);
        assert!(
            v["connection_capabilities"]["operation_constraints"]["data.import"]["schema"]["oneOf"]
                .is_array()
        );
    }
    assert_eq!(app.session.project_context, context);
    assert!(!app.doc.dirty);
    assert!(app.doc.automation_runs.is_empty());
    app.start_new_project();
    app.session.ui.phase_edit_active = true;
    let state = s.call(&mut app, "plotium_get_state", json!({})).unwrap();
    assert_eq!(state["project_present"], true);
    assert!(state["project"].is_null() && state["selection"].is_null());
    assert!(app.doc.automation_runs.is_empty());
    assert!(!app.doc.dirty);
}

#[test]
fn frozen_jdf_from_welcome_keeps_science_context_and_all_ids_through_undo_redo() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("synthetic.jdf");
    std::fs::copy(fixture("jeol-diffusion.jdf"), &path).unwrap();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut s = session();
    let context = app.session.project_context;
    let p = plan(&mut s, &mut app, std::slice::from_ref(&path));
    assert!(!app.session.project_present);
    assert!(!app.doc.dirty);
    assert!(app.doc.automation_runs.is_empty());
    let proposed = &p["import"]["items"][0]["metadata"];
    std::fs::write(&path, b"changed since preflight").unwrap();
    std::fs::remove_file(path).unwrap();
    let result = execute(&mut s, &mut app, &p).unwrap();
    assert_eq!(result["value"]["status"], "succeeded");
    assert_eq!(
        json!(app.doc.datasets[0].resource_id()),
        proposed["dataset_id"]
    );
    assert_eq!(
        json!(app.doc.canvases[0].resource_id),
        proposed["canvas_id"]
    );
    assert!(app.session.project_present);
    assert_eq!(app.session.project_context, context);
    let d = app.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(d.data.rows, 14);
    for (i, g) in d
        .data
        .pseudo_axis
        .as_ref()
        .unwrap()
        .values
        .iter()
        .enumerate()
    {
        assert!((g - 0.02 * (i + 1) as f64).abs() < 1e-14);
    }
    let mut input = d.suggested_diffusion_input().unwrap();
    let DiffusionModel::JeolBppDste {
        delta,
        big_delta,
        tau_1,
        tau_2,
        ..
    } = input.model
    else {
        panic!("wrong diffusion model")
    };
    for (actual, expected) in [
        (delta, 0.0035),
        (big_delta, 0.1),
        (tau_1, 0.001),
        (tau_2, 0.001),
    ] {
        assert!((actual - expected).abs() < 1e-14);
    }
    input.confirmed = true;
    for (i, b) in input.b_values(14).unwrap().iter().enumerate() {
        let expected = 33_899_436.515_824_9 * ((i + 1) as f64).powi(2);
        assert!((b / expected - 1.0).abs() < 1e-12);
    }
    let canvas = plotium_core::automation::document_root_target();
    let ids = app.doc.canvases[0]
        .panels
        .iter()
        .map(|p| p.id)
        .collect::<Vec<_>>();
    let revision = app.doc.automation_revision;
    let search = s
        .call(
            &mut app,
            "plotium_resources_search",
            json!({"expected_revision":revision,"expected_project_context":context,"query":{}}),
        )
        .unwrap();
    assert!(
        search
            .to_string()
            .contains(proposed["dataset_id"].as_str().unwrap())
    );
    app.undo();
    assert!(app.doc.datasets.is_empty() && app.doc.canvases.is_empty());
    assert!(app.session.project_present);
    app.redo();
    assert_eq!(
        json!(app.doc.datasets[0].resource_id()),
        proposed["dataset_id"]
    );
    assert_eq!(
        json!(app.doc.canvases[0].resource_id),
        proposed["canvas_id"]
    );
    assert_eq!(
        app.doc.canvases[0]
            .panels
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(plotium_core::automation::document_root_target(), canvas);
    assert_eq!(
        execute(&mut s, &mut app, &p).unwrap_err().code,
        "expired_plan"
    );
}

#[test]
fn generic_loader_handles_jcamp_atomic_bruker_directory_and_partial_failure() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut s = session();
    let missing = fixture("jcamp-hz.dx").with_file_name("missing.jdf");
    let p = plan(
        &mut s,
        &mut app,
        &[
            fixture("jcamp-hz.dx"),
            fixture("bruker-1d/pdata/1"),
            missing.clone(),
        ],
    );
    assert_eq!(p["import"]["status"], "partial");
    assert_eq!(p["import"]["prepared_files"], 2, "{p}");
    let r = execute(&mut s, &mut app, &p).unwrap();
    assert_eq!(r["value"]["status"], "partial");
    assert_eq!(app.doc.datasets.len(), 2);
    assert_eq!(app.doc.canvases.len(), 2);
    let first = app.doc.datasets[0].resource_id();
    let p = plan(&mut s, &mut app, &[fixture("jeol-complex.jdf")]);
    execute(&mut s, &mut app, &p).unwrap();
    assert_eq!(app.doc.datasets.len(), 3);
    app.undo();
    assert_eq!(app.doc.datasets.len(), 2);
    assert_eq!(app.doc.datasets[0].resource_id(), first);
    let mut welcome = PlotiumApp::new_with_settings(Default::default());
    let p = plan(&mut s, &mut welcome, &[missing]);
    assert_eq!(
        execute(&mut s, &mut welcome, &p).unwrap()["value"]["status"],
        "failed"
    );
    assert!(!welcome.session.project_present);
    assert!(!welcome.doc.dirty);
    assert!(welcome.doc.automation_runs.is_empty());
}

#[test]
fn native_plans_recheck_authority_revision_context_limits_and_single_use() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut s = session();
    let paths = [fixture("jeol-complex.jdf")];
    let p = plan(&mut s, &mut app, &paths);
    s.allow_modify = false;
    assert_eq!(
        execute(&mut s, &mut app, &p).unwrap_err().code,
        "permission_denied"
    );
    s.allow_modify = true;
    assert_eq!(
        execute(&mut s, &mut app, &p).unwrap_err().code,
        "expired_plan"
    );
    let p = plan(&mut s, &mut app, &paths);
    app.doc.automation_revision += 1;
    assert_eq!(
        execute(&mut s, &mut app, &p).unwrap_err().code,
        "stale_revision"
    );
    let p = plan(&mut s, &mut app, &paths);
    app.start_new_project();
    assert_eq!(
        execute(&mut s, &mut app, &p).unwrap_err().code,
        "project_changed"
    );
    for _ in 0..4 {
        plan(&mut s, &mut app, &paths);
    }
    let a = args(&app, &paths);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "plan_limit"
    );
    let mut s = session();
    let a = args(&app, &vec![paths[0].clone(); 17]);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "invalid_parameters"
    );
    let mut a = args(&app, &paths);
    a["parameters"]["mode"] = json!("unknown");
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "invalid_parameters"
    );
}

#[test]
fn background_native_preflight_keeps_state_responsive_and_discards_cancelled_or_old_work() {
    for switch in [false, true] {
        let mut c = Connection::start(|| {}).unwrap();
        c.session.allow_modify = true;
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let cancellation = tokio_util::sync::CancellationToken::new();
        let req = PendingRequest {
            name: "plotium_plan".into(),
            arguments: args(&app, &[fixture("jeol-diffusion.jdf")]),
            response: tx,
            cancellation: cancellation.clone(),
            transport: None,
        };
        c.dispatch_request(&mut app, req, false).unwrap();
        assert!(rx.try_recv().is_err());
        assert!(
            !c.session
                .call(&mut app, "plotium_get_state", json!({}))
                .unwrap()["project_present"]
                .as_bool()
                .unwrap()
        );
        if switch {
            app.start_new_project();
            c.synchronize_project(&app);
        } else {
            cancellation.cancel();
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        while c.import.is_some() {
            assert!(Instant::now() < deadline);
            if let Err(e) = c.poll_import(&mut app, false) {
                assert_eq!(e.code, "request_cancelled");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            rx.blocking_recv().unwrap().unwrap_err().code,
            if switch {
                "project_changed"
            } else {
                "request_cancelled"
            }
        );
        assert!(app.doc.datasets.is_empty());
        assert!(!app.doc.dirty);
    }
}

#[test]
fn frozen_native_budget_rejects_oversize_without_installing_resources() {
    use plotium_core::automation::{AutomationError, DataImportParams, PreparedDataImport};
    let params = DataImportParams::parse(json!({"paths":[fixture("jeol-diffusion.jdf")]})).unwrap();
    assert!(matches!(
        PreparedDataImport::prepare_with_budget(params, 1, || false),
        Err(AutomationError::NativeImportBudget(_))
    ));
    let params = DataImportParams::parse(json!({"paths":[fixture("jeol-diffusion.jdf")]})).unwrap();
    assert!(matches!(
        PreparedDataImport::prepare(params, || true),
        Err(AutomationError::PreviewCancelled)
    ));
    let path = fixture("jeol-diffusion.jdf").with_file_name("x".repeat(256 * 1024));
    let params = DataImportParams::parse(json!({"paths":[path]})).unwrap();
    assert!(matches!(
        PreparedDataImport::prepare(params, || false),
        Err(AutomationError::ResponseTooLarge)
    ));
}

#[test]
fn native_reservations_are_shared_and_released_on_every_plan_consumption() {
    use plotium_core::automation::NATIVE_IMPORT_BYTES;
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut s = session();
    let paths = [fixture("jeol-complex.jdf")];
    let a = plan(&mut s, &mut app, &paths);
    let b = plan(&mut s, &mut app, &paths);
    let a_bytes = a["import"]["reserved_bytes"].as_u64().unwrap() as usize;
    let b_bytes = b["import"]["reserved_bytes"].as_u64().unwrap() as usize;
    assert_eq!(
        s.remaining_import_bytes(),
        NATIVE_IMPORT_BYTES - a_bytes - b_bytes
    );
    execute(&mut s, &mut app, &a).unwrap();
    assert_eq!(s.remaining_import_bytes(), NATIVE_IMPORT_BYTES - b_bytes);
    assert_eq!(
        execute(&mut s, &mut app, &b).unwrap_err().code,
        "stale_revision"
    );
    assert_eq!(s.remaining_import_bytes(), NATIVE_IMPORT_BYTES);
}

#[test]
fn real_mcp_imports_jdf_from_welcome_and_discovers_produced_resources() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let mut c = Connection::start(|| {}).unwrap();
    c.session.allow_modify = true;
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut config = StreamableHttpClientTransportConfig::with_uri(c.url.clone());
    config.auth_header = Some(c.token.clone());
    let path = fixture("jeol-diffusion.jdf");
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let client=tokio::spawn(async move {
            let http=reqwest::Client::builder().no_proxy().build().unwrap();
            let client=().serve(StreamableHttpClientTransport::with_client(http,config)).await.unwrap();
            async fn call(client: &rmcp::service::RunningService<rmcp::RoleClient,()>, name: &str, args: Value) -> Value {
                let r=client.call_tool(CallToolRequestParams::new(name.to_owned()).with_arguments(args.as_object().unwrap().clone())).await.unwrap();
                assert_eq!(r.is_error,Some(false),"{r:?}");
                r.structured_content.unwrap()
            }
            let state=call(&client,"plotium_get_state",json!({})).await;
            assert_eq!(state["project_present"],false);
            let p=call(&client,"plotium_plan",json!({"tool_id":"data.import","parameters":{"paths":[path]},"expected_revision":state["revision"],"expected_project_context":state["project_context"]})).await;
            assert_eq!(p["import"]["items"][0]["metadata"]["inspection"]["dimension"]["shape"][0],14);
            let r=call(&client,"plotium_execute",json!({"plan_id":p["plan_id"]})).await;
            assert_eq!(r["value"]["status"],"succeeded");
            assert_eq!(r["produced"].as_array().unwrap().len(),2);
            let resources=call(&client,"plotium_resources_search",json!({"expected_revision":r["after_revision"],"expected_project_context":state["project_context"],"query":{}})).await;
            for produced in r["produced"].as_array().unwrap() { assert!(resources.to_string().contains(produced["id"].as_str().unwrap())); }
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(30),async {
            while !client.is_finished() {
                if let Some(req)=c.next_request() {c.dispatch_request(&mut app,req,false).unwrap();}
                c.poll_import(&mut app,false).unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            client.await.unwrap();
        }).await.unwrap();
    });
    let d = app.doc.datasets[0].as_nmr2d().unwrap();
    assert_eq!(d.data.rows, 14);
    assert_eq!(
        d.data
            .jeol_diffusion
            .as_ref()
            .unwrap()
            .seconds("diffusion_time"),
        Some(0.1)
    );
}
