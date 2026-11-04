use crate::Session;
use plotium_core::state::PlotiumApp;
use serde_json::{Value, json};

fn execute(session: &mut Session, app: &mut PlotiumApp, tool: &str, parameters: Value) -> Value {
    let request = json!({"tool_id":tool,"parameters":parameters,
        "ids":[app.doc.datasets[0].resource_id().to_string()],
        "expected_revision":app.doc.automation_revision,
        "expected_project_context":app.session.project_context});
    let plan = session.call(app, "plotium_plan", request).unwrap();
    session
        .call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap()
}
#[test]
fn mcp_peak_edits_and_readonly_export_match_desktop_core() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(
        plotium_io::load_path(path).unwrap().acquisition,
    )
    .unwrap();
    let owner = dataset.resource_id();
    let volume = dataset.as_nmr2d().unwrap().volume.as_ref().unwrap();
    let ppm: [f64; 3] = std::array::from_fn(|a| volume.processed.axis_coordinate(a, 1).unwrap());
    let mut direct = PlotiumApp::new();
    direct.doc.datasets.push(dataset.clone());
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    let mut session = Session::default();
    session.set_allow_modify(true);
    let id = direct.add_peak_3d(owner, ppm).unwrap();
    execute(
        &mut session,
        &mut app,
        "nmr.peaks3d",
        json!({"operation":"add","ppm":ppm}),
    );
    direct
        .edit_peak_3d(owner, id, ppm, "overlap".into())
        .unwrap();
    execute(
        &mut session,
        &mut app,
        "nmr.peaks3d",
        json!({"operation":"edit","id":id,"note":"overlap"}),
    );
    direct.locate_peak_3d(owner, id).unwrap();
    execute(
        &mut session,
        &mut app,
        "nmr.peaks3d",
        json!({"operation":"locate","id":id}),
    );
    assert_eq!(
        app.session.ui.selected_peak_3d,
        direct.session.ui.selected_peak_3d
    );
    session.set_allow_modify(false);
    let revision = app.doc.automation_revision;
    let query = json!({"ids":[owner.to_string()],"expected_revision":revision,
        "expected_project_context":app.session.project_context});
    let result = session
        .call(&mut app, "plotium_peaks3d_inspect", query)
        .unwrap();
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(result["value"], direct.peak_table_3d(owner).unwrap());
    session.set_allow_modify(true);
    direct.delete_peak_3d(owner, id).unwrap();
    let result = execute(
        &mut session,
        &mut app,
        "nmr.peaks3d",
        json!({"operation":"delete","id":id}),
    );
    assert_eq!(result["value"], direct.peak_table_3d(owner).unwrap());
    app.undo();
    direct.undo();
    assert_eq!(
        app.peak_table_3d(owner).unwrap(),
        direct.peak_table_3d(owner).unwrap()
    );
}
