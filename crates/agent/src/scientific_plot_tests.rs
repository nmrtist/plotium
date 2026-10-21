//! Original synthetic acquisition: exercise the same MCP calls as an agent.
use crate::session::Session;
use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const D: f64 = 1.2e-9;
#[path = "external_fit_acceptance_tests.rs"]
mod external_acceptance;
#[path = "scientific_plot_regression_tests.rs"]
mod regressions;
fn acquisition(dir: &std::path::Path) -> PathBuf {
    let mut blob = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/jeol-diffusion.jdf"),
    )
    .unwrap();
    let old_start = u32::from_be_bytes(blob[1284..1288].try_into().unwrap()) as usize;
    // Original calibration records: 400 MHz, zero carrier, 1 kHz sweep;
    // preserve unknown filter evidence so import leaves processing explicit.
    let mut extra = vec![0u8; 4 * 64];
    for (i, (name, value)) in [("x_freq", 400e6_f64), ("x_offset", 0.), ("x_sweep", 1000.)]
        .into_iter()
        .enumerate()
    {
        let record = &mut extra[i * 64..(i + 1) * 64];
        record[6..8].copy_from_slice(&[1, 13]);
        record[16..24].copy_from_slice(&value.to_le_bytes());
        record[32..36].copy_from_slice(&2_i32.to_le_bytes());
        record[36..36 + name.len()].copy_from_slice(name.as_bytes());
    }
    extra[3 * 64 + 16..3 * 64 + 23].copy_from_slice(b"UNKNOWN");
    extra[3 * 64 + 36..3 * 64 + 50].copy_from_slice(b"digital_filter");
    blob.splice(old_start..old_start, extra);
    let start = old_start + 256;
    blob[1284..1288].copy_from_slice(&(start as u32).to_be_bytes());
    let parameter_length = (start - 1360) as u32;
    blob[1216..1220].copy_from_slice(&parameter_length.to_be_bytes());
    blob[1372..1376].copy_from_slice(&parameter_length.to_le_bytes());
    let count = u32::from_le_bytes(blob[1368..1372].try_into().unwrap()) + 4;
    blob[1368..1372].copy_from_slice(&count.to_le_bytes());
    blob[1064..1072].copy_from_slice(&400_f64.to_be_bytes());
    for row in 0..14 {
        for col in 0..4 {
            let value = if col == 0 {
                // Small deterministic acquisition noise exercises nonzero-cost
                // fitting and finite uncertainty through the complete pipeline.
                1000. * (-D * row as f64 * 1e8).exp() + (row % 5) as f64 * 0.002
            } else {
                0.
            };
            let offset = start + (row * 4 + col) * 8;
            blob[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    let path = dir.join("original-decay.jdf");
    std::fs::write(&path, blob).unwrap();
    path
}
fn call(s: &mut Session, a: &mut PlotiumApp, name: &str, mut v: Value) -> Value {
    v["expected_revision"] = json!(a.doc.automation_revision);
    v["expected_project_context"] = json!(a.session.project_context);
    let before = (
        a.doc.dirty,
        a.doc.edit_generation,
        a.doc.automation_revision,
        a.doc.automation_runs.len(),
    );
    let result = s
        .call(a, name, v)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    if matches!(
        name,
        "plotium_processing_inspect"
            | "plotium_diffusion_inspect"
            | "plotium_analysis_results_inspect"
    ) {
        assert_eq!(
            (
                a.doc.dirty,
                a.doc.edit_generation,
                a.doc.automation_revision,
                a.doc.automation_runs.len()
            ),
            before,
            "{name}"
        );
    }
    result
}
fn edit(
    s: &mut Session,
    a: &mut PlotiumApp,
    tool: &str,
    params: Value,
    ids: Option<Value>,
) -> Value {
    let mut args = json!({"tool_id":tool,"parameters":params});
    if let Some(ids) = ids {
        args["ids"] = ids;
    }
    let plan = call(s, a, "plotium_plan", args);
    s.call(a, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap()
}
fn settle(s: &mut Session, a: &mut PlotiumApp, result: &Value) -> Value {
    let id = &result["value"]["task"]["task_id"];
    let deadline = Instant::now() + Duration::from_secs(15);
    while a.compute_busy() && Instant::now() < deadline {
        a.poll_compute();
        std::thread::sleep(Duration::from_millis(1));
    }
    a.poll_compute();
    let task = s
        .call(a, "plotium_tasks_get", json!({"task_id":id}))
        .unwrap();
    assert_eq!(task["task"]["status"], "succeeded", "{task}");
    task["task"].clone()
}
fn resource(a: &PlotiumApp, kind: &str, key: &str) -> ResourceRef {
    ProjectResourceProvider::new(a)
        .descriptors()
        .into_iter()
        .find(|d| d.resource.kind.0 == kind && d.resource.local_id.as_deref() == Some(key))
        .unwrap()
        .resource
}
fn input() -> Value {
    json!({"model":{"model":"direct_b","unit":"s/m²"},"rule_version":1,"source":"Original synthetic test: b_i = i × 10^8 s/m², D = 1.2 × 10^-9 m²/s","confirmed":true,
        "increments":(0..14).map(|i|json!({"id":i,"b":i as f64*1e8})).collect::<Vec<_>>(),"gradients":null,"vendor_coefficient_seconds":null,"program_sha256":null})
}

#[test]
fn native_decay_mono_exp_and_ilt_plots_check_science_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let path = acquisition(dir.path());
    let mut a = PlotiumApp::new_with_settings(Default::default());
    let mut s = Session::default();
    s.set_allow_modify(true);
    let imported = edit(&mut s, &mut a, "data.import", json!({"paths":[path]}), None);
    assert_eq!(imported["value"]["status"], "succeeded", "{imported}");
    let source = resource(&a, KIND_FIELD, "nmr.stack");
    let owner = source.parent_id.clone().unwrap();
    let processed = edit(
        &mut s,
        &mut a,
        PROCESSING_EDIT,
        json!({"edits":[{"op":"add","axis":"f2","kind":"Fft"}]}),
        Some(json!([owner])),
    );
    settle(&mut s, &mut a, &processed);
    let encoding = input();
    let checked = call(
        &mut s,
        &mut a,
        "plotium_diffusion_inspect",
        json!({"ids":[source.id],"input":encoding}),
    );
    assert_eq!(checked["value"]["executable"], true, "{checked}");
    // The fixture contains one flat spectral component. Explicit rule: Height
    // over the entire acquired band, using inclusive bounds wider than the band.
    let extracted = edit(
        &mut s,
        &mut a,
        REGIONS_EXTRACT,
        json!({"regions":[{"source":"explicit","bounds":[-1e9,1e9],"metric":"Height","name":"Whole-band synthetic resonance"}],"input":encoding}),
        Some(json!([source.id])),
    );
    let extracted = settle(&mut s, &mut a, &extracted);
    let table_id = extracted["produced"][0]["id"].clone();
    let table = a
        .doc
        .datasets
        .iter()
        .find(|d| json!(d.resource_id()) == table_id)
        .unwrap()
        .as_table()
        .unwrap();
    let column = table.series_bindings[0].value_column;
    assert!(table.meta.region_extraction.is_some());
    regressions::check_unconverged_fit(&mut s, &mut a, table_id.clone(), json!(column));
    let fitted = edit(
        &mut s,
        &mut a,
        DIFFUSION_FIT,
        json!({"response_column":column}),
        Some(json!([table_id])),
    );
    let fitted = settle(&mut s, &mut a, &fitted);
    let analysis: ResourceRef = serde_json::from_value(fitted["produced"][0].clone()).unwrap();
    let report = call(
        &mut s,
        &mut a,
        "plotium_analysis_results_inspect",
        json!({"ids":[analysis.id]}),
    );
    assert_eq!(report["value"]["valid"], true, "{report}");
    let fit = &report["value"]["analyses"][0];
    let estimate = fit["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["parameter"] == "D")
        .unwrap();
    assert!(
        (estimate["value"].as_f64().unwrap() / D - 1.).abs() < 1e-4,
        "{report}"
    );
    assert!(estimate["standard_error"].as_f64().unwrap().is_finite());
    assert!(fit["quality"]["r_squared"].as_f64().unwrap() > 0.99999);
    assert_eq!(fit["quality"]["observations"], 14);
    assert!(fit["termination"][0]["converged"] == true, "{report}");
    let plotted = edit(
        &mut s,
        &mut a,
        DIFFUSION_PLOT_CREATE,
        json!({"source":analysis,"name":"Synthetic decay fit"}),
        None,
    );
    assert_eq!(plotted["value"]["evidence"]["valid"], true);
    check_plot(&mut s, &mut a, &plotted, true);
    regressions::reject_failed_selected_attempt(&mut s, &mut a, &analysis);
    for method in ["mono_exp", "ilt"] {
        let mut params = json!({"method":method,"input":encoding});
        if method == "ilt" {
            params["params"] = json!({"d_min":1e-10,"d_max":1e-8,"n_grid":32,"lambda":0.001});
        }
        let built = edit(&mut s, &mut a, DOSY_BUILD, params, Some(json!([source.id])));
        settle(&mut s, &mut a, &built);
        let result = resource(
            &a,
            KIND_FIELD,
            if method == "ilt" {
                "nmr.ilt_map"
            } else {
                "nmr.dosy_map"
            },
        );
        let report = call(
            &mut s,
            &mut a,
            "plotium_analysis_results_inspect",
            json!({"ids":[result.id]}),
        );
        assert_eq!(report["value"]["valid"], true, "{report}");
        assert!(
            report["value"]["accepted_columns"].as_u64().unwrap() > 0,
            "{report}"
        );
        assert!(report["value"]["provenance"]["encoding"].is_object());
        if method == "mono_exp" {
            for col in report["value"]["columns"].as_array().unwrap() {
                if let Some(d) = col["d"].as_f64() {
                    assert!((d / D - 1.).abs() < 1e-4);
                }
            }
        }
        let plotted = edit(
            &mut s,
            &mut a,
            FIELD_PLOT_CREATE,
            json!({"source":result,"name":format!("Synthetic {method} DOSY")}),
            None,
        );
        check_plot(&mut s, &mut a, &plotted, false);
    }
}
fn check_plot(s: &mut Session, a: &mut PlotiumApp, plotted: &Value, fit: bool) {
    let canvas = plotted["value"]["canvas"].clone();
    let page = a.doc.canvases.last().unwrap();
    let id = page.resource_id;
    let size_mm = page.size_mm;
    let figures = page
        .objects
        .iter()
        .filter_map(|o| o.plot())
        .map(|p| serde_json::to_value(p.figure()).unwrap())
        .collect::<Vec<_>>();
    if fit {
        assert_eq!(page.panels.len(), 2);
        let plot = page.objects.iter().find_map(|o| o.plot()).unwrap();
        assert_eq!(plot.binding.series.len(), 2);
        assert!(plot.figure().series.iter().any(|s| s.points.len() > 100));
        assert!(page.objects.iter().any(|o|matches!(&o.kind,ContentKind::Text(t) if t.text.contains("D =") && t.text.contains("±"))));
    } else {
        let plot = page.objects[0].plot().unwrap();
        assert!(plot.display_owner.is_none());
        assert!(!plot.figure().contours.is_empty());
    }
    let mut diagnostic = call(
        s,
        a,
        "plotium_figure_diagnose",
        json!({"canvas":canvas,"output":{"size_mm":size_mm}}),
    );
    let mut diagnostic_rows = Vec::new();
    loop {
        diagnostic_rows.extend(
            diagnostic["value"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
        if diagnostic["value"]["coverage"]["scan_complete"] == true {
            break;
        }
        let cursor = diagnostic["value"]["continuation"].clone();
        diagnostic = call(s, a, "plotium_figure_diagnose", json!({"cursor":cursor}));
    }
    let issues = diagnostic_rows
        .iter()
        .filter(|r| r["kind"] == "issue")
        .collect::<Vec<_>>();
    assert!(issues.is_empty(), "{issues:?}");
    let preview = call(
        s,
        a,
        "plotium_render_preview",
        json!({"target":canvas,"max_edge":640}),
    );
    if let Some(dir) = std::env::var_os("PLOTIUM_SCIENCE_ACCEPTANCE_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = a.doc.canvases.last().unwrap().name.replace(' ', "_");
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_vec(
                &json!({"preview":preview,"diagnostic":diagnostic,"result":plotted}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    assert_eq!(preview["value"]["mime_type"], "image/png");
    assert!(preview["value"]["width_px"].as_u64().unwrap() > 0);
    let produced = plotted["produced"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    a.undo();
    assert!(a.doc.canvas_index(id).is_none());
    assert!(
        produced
            .iter()
            .all(|id| ProjectResourceProvider::new(a).inspect(id).is_none())
    );
    a.redo();
    assert!(a.doc.canvas_index(id).is_some());
    assert!(
        produced
            .iter()
            .all(|id| ProjectResourceProvider::new(a).inspect(id).is_some())
    );
    let restored = a.doc.canvases.iter().find(|p| p.resource_id == id).unwrap();
    assert_eq!(
        figures,
        restored
            .objects
            .iter()
            .filter_map(|o| o.plot())
            .map(|p| serde_json::to_value(p.figure()).unwrap())
            .collect::<Vec<_>>()
    );
}

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;
async fn http_call(client: &Client, name: &str, args: Value) -> Value {
    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false), "{name}: {result:?}");
    result.structured_content.unwrap()
}
async fn http_read(client: &Client, name: &str, mut args: Value) -> Value {
    let state = http_call(client, "plotium_get_state", json!({})).await;
    args["expected_revision"] = state["revision"].clone();
    args["expected_project_context"] = state["project_context"].clone();
    http_call(client, name, args).await
}
async fn http_edit(client: &Client, tool: &str, parameters: Value, ids: Option<Value>) -> Value {
    let mut args = json!({"tool_id":tool,"parameters":parameters});
    if let Some(ids) = ids {
        args["ids"] = ids;
    }
    let plan = http_read(client, "plotium_plan", args).await;
    http_call(
        client,
        "plotium_execute",
        json!({"plan_id":plan["plan_id"]}),
    )
    .await
}
async fn http_wait(client: &Client, result: &Value) -> Value {
    let task_id = &result["value"]["task"]["task_id"];
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < deadline, "Task did not terminate");
        let receipt = http_call(client, "plotium_tasks_get", json!({"task_id":task_id})).await;
        if receipt["task"]["status"] == "succeeded" {
            return receipt["task"].clone();
        }
        assert!(
            !matches!(
                receipt["task"]["status"].as_str(),
                Some("failed" | "cancelled" | "stale")
            ),
            "{receipt}"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
async fn http_field(client: &Client, key: &str) -> Value {
    let resources = http_read(
        client,
        "plotium_resources_search",
        json!({"query":{"kinds":[KIND_FIELD]}}),
    )
    .await;
    resources["value"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["local_id"] == key)
        .unwrap()
        .clone()
}
async fn http_preview(client: &Client, canvas: &Value, size_mm: Value) {
    let detail = http_read(
        client,
        "plotium_document_inspect",
        json!({"target":canvas,"view":"self"}),
    )
    .await;
    assert_eq!(detail["value"]["node"]["target"], *canvas);
    let mut diagnosed = http_read(
        client,
        "plotium_figure_diagnose",
        json!({"canvas":canvas,"output":{"size_mm":size_mm}}),
    )
    .await;
    loop {
        let issues = diagnosed["value"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "issue")
            .collect::<Vec<_>>();
        assert!(issues.is_empty(), "{issues:?}");
        if diagnosed["value"]["coverage"]["scan_complete"] == true {
            break;
        }
        diagnosed = http_read(
            client,
            "plotium_figure_diagnose",
            json!({"cursor":diagnosed["value"]["continuation"]}),
        )
        .await;
    }
    let state = http_call(client, "plotium_get_state", json!({})).await;
    let preview = client.call_tool(rmcp::model::CallToolRequestParams::new("plotium_render_preview").with_arguments(json!({"expected_project_context":state["project_context"],"expected_revision":state["revision"],"target":canvas,"max_edge":640}).as_object().unwrap().clone())).await.unwrap();
    assert_eq!(preview.is_error, Some(false), "{preview:?}");
    assert_eq!(
        preview
            .content
            .iter()
            .filter(|c| c.as_image().is_some())
            .count(),
        1
    );
    assert_eq!(
        preview.structured_content.unwrap()["value"]["render_revision"],
        state["revision"]
    );
}

#[test]
fn real_http_mcp_processing_fit_and_both_dosy_native_pngs() {
    use rmcp::{
        ServiceExt,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let dir = tempfile::tempdir().unwrap();
    let path = acquisition(dir.path());
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut connection = crate::Connection::start(|| {}).unwrap();
    connection.session.set_allow_modify(true);
    let mut config = StreamableHttpClientTransportConfig::with_uri(connection.url.clone());
    config.auth_header = Some(connection.token.clone());
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let task = tokio::spawn(async move {
            let client=().serve(StreamableHttpClientTransport::with_client(reqwest::Client::builder().no_proxy().build().unwrap(),config)).await.unwrap();
            let state = http_call(&client,"plotium_capabilities_get",json!({})).await;
            assert!(state["connection_capabilities"]["operation_constraints"][FIELD_PLOT_CREATE]["schema"].is_object());
            let imported = http_edit(&client,"data.import",json!({"paths":[path]}),None).await;
            assert_eq!(imported["value"]["status"],"succeeded");
            let source = http_field(&client,"nmr.stack").await;
            let processed = http_edit(&client,PROCESSING_EDIT,json!({"edits":[{"op":"add","axis":"f2","kind":"Fft"}]}),Some(json!([source["parent_id"]]))).await;
            http_wait(&client,&processed).await;
            let encoding = input();
            let report = http_read(&client,"plotium_diffusion_inspect",json!({"ids":[source["id"]],"input":encoding})).await;
            assert_eq!(report["value"]["executable"],true);
            let extracted = http_edit(&client,REGIONS_EXTRACT,json!({"regions":[{"source":"explicit","bounds":[-1e9,1e9],"metric":"Height","name":"Whole-band synthetic resonance"}],"input":encoding}),Some(json!([source["id"]]))).await;
            let extracted = http_wait(&client,&extracted).await;
            let fitted = http_edit(&client,DIFFUSION_FIT,json!({"response_column":extracted["summary"]["columns"][0]["column"]}),Some(json!([extracted["produced"][0]["id"]]))).await;
            let fitted = http_wait(&client,&fitted).await;
            let analysis = &fitted["produced"][0];
            let report = http_read(&client,"plotium_analysis_results_inspect",json!({"ids":[analysis["id"]]})).await;
            assert_eq!(report["value"]["valid"],true);
            let fit = &report["value"]["analyses"][0];
            let d = fit["parameters"].as_array().unwrap().iter().find(|p|p["parameter"]=="D").unwrap();
            assert!((d["value"].as_f64().unwrap()/D-1.).abs()<1e-4);
            assert!(fit["quality"]["r_squared"].as_f64().unwrap()>0.99999);
            assert_eq!(fit["quality"]["observations"],14);
            assert_eq!(fit["termination"][0]["converged"],true);
            assert!(report["value"]["source"].is_object());
            let plotted = http_edit(&client,DIFFUSION_PLOT_CREATE,json!({"source":analysis,"name":"MCP native decay fit"}),None).await;
            http_preview(&client,&plotted["value"]["canvas"],json!([180.,150.])).await;
            for method in ["mono_exp","ilt"] {
                let mut parameters=json!({"method":method,"input":encoding});
                if method=="ilt" {parameters["params"]=json!({"d_min":1e-10,"d_max":1e-8,"n_grid":32,"lambda":0.001});}
                let built = http_edit(&client,DOSY_BUILD,parameters,Some(json!([source["id"]]))).await;
                http_wait(&client,&built).await;
                let field = http_field(&client,if method=="ilt" {"nmr.ilt_map"}else{"nmr.dosy_map"}).await;
                let report = http_read(&client,"plotium_analysis_results_inspect",json!({"ids":[field["id"]]})).await;
                assert_eq!(report["value"]["valid"],true);
                assert!(report["value"]["accepted_columns"].as_u64().unwrap()>0);
                assert!(report["value"]["provenance"]["data_fingerprint"].is_string());
                let plotted = http_edit(&client,FIELD_PLOT_CREATE,json!({"source":field,"name":format!("MCP {method} DOSY")}),None).await;
                http_preview(&client,&plotted["value"]["canvas"],json!(DEFAULT_CANVAS_SIZE_MM)).await;
            }
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(60),async {
            while !task.is_finished() {
                while let Some(request)=connection.next_request() {connection.dispatch_request(&mut app,request,false).unwrap();}
                connection.poll_import(&mut app,false).unwrap();
                app.poll_compute();
                connection.poll_diagnostic(&mut app, false).unwrap();
                connection.poll_preview(&mut app).unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            task.await.unwrap();
        }).await.unwrap();
    });
    assert_eq!(app.doc.canvases.len(), 4);
    assert_eq!(app.doc.datasets.len(), 3);
}
