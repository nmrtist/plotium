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

#[test]
fn mcp_protein_uses_document_target_permissions_and_revision() {
    let mut app = PlotiumApp::new();
    let mut session = Session::default();
    let context = app.session.project_context;
    let request = json!({
        "tool_id":"protein.edit",
        "parameters":{"operation":"import","source":"synthetic.fasta","fasta":">A\nAG\n>B\nG"},
        "expected_revision":0,"expected_project_context":context
    });
    assert!(
        session
            .call(&mut app, "plotium_plan", request.clone())
            .is_err()
    );
    session.set_allow_modify(true);
    let plan = session
        .call(&mut app, "plotium_plan", request.clone())
        .unwrap();
    session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert!(session.call(&mut app, "plotium_plan", request).is_err());
    assert_eq!(app.doc.protein.molecules.len(), 1);
    let id = app.doc.protein.molecules[0].id;
    let residue = app.doc.protein.molecules[0].chains[0].residues[0].id;
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(
        plotium_io::load_path(fixture).unwrap().acquisition,
    )
    .unwrap();
    let d = dataset.resource_id();
    app.doc.datasets.push(dataset);
    let v = plotium_core::protein::volume(&app.doc, d).unwrap();
    let ppm = std::array::from_fn(|axis| v.processed.axis_coordinate(axis, 1).unwrap());
    let peak = app.add_peak_3d(d, ppm).unwrap();
    let edit = |session: &mut Session, app: &mut PlotiumApp, tool: &str, parameters: Value| {
        let request = json!({"tool_id":tool,"parameters":parameters,
            "expected_revision":app.doc.automation_revision,"expected_project_context":context});
        let plan = session.call(app, "plotium_plan", request).unwrap();
        session
            .call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
            .unwrap()
    };
    edit(
        &mut session,
        &mut app,
        "protein.edit",
        json!({"operation":"bind","dataset":d,"molecule":id}),
    );
    let added = edit(
        &mut session,
        &mut app,
        "protein.assignments.edit",
        json!({"dataset":d,"peak":peak,"edit":{"operation":"add","targets":[{"residue":residue,"members":["H"]},null,null]}}),
    );
    let candidate = &added["value"]["assignment"]["candidates"][0]["id"];
    edit(
        &mut session,
        &mut app,
        "protein.assignments.edit",
        json!({"dataset":d,"peak":peak,"edit":{"operation":"confirm","candidate":candidate}}),
    );
    assert_eq!(
        json!(app.doc.protein.assignment(d, peak).unwrap().confirmed),
        *candidate
    );
    session.set_allow_modify(false);
    for (name, parameters) in [
        ("plotium_protein_inspect", json!({"molecule":id})),
        ("plotium_protein_assignments_export", json!({"molecule":id})),
    ] {
        let query = json!({"parameters":parameters,"expected_revision":app.doc.automation_revision,"expected_project_context":context});
        let result = session.call(&mut app, name, query).unwrap();
        if name.ends_with("export") {
            assert_eq!(result["value"], app.export_protein(id, false).unwrap());
        } else {
            assert_eq!(result["value"]["molecules"][0]["id"], json!(id));
        }
    }
}

#[test]
fn mcp_protein_inspection_can_page_a_sequence_larger_than_the_response_budget() {
    use plotium_core::protein::Edit;
    let mut app = PlotiumApp::new();
    app.edit_protein(Edit::Import {
        source: "large.fasta".into(),
        fasta: format!(">large\n{}G", "A".repeat(6_000)),
        name: None,
        chains: None,
    })
    .unwrap();
    let m = &app.doc.protein.molecules[0];
    assert!(
        serde_json::to_vec(m).unwrap().len() > plotium_core::automation::DOCUMENT_RESPONSE_BYTES
    );
    let molecule = m.id;
    let chain = m.chains[0].id;
    let last = m.chains[0].residues.last().unwrap().id;
    let mut session = Session::default();
    let mut read = |parameters: Value| {
        let query = json!({"parameters":parameters,"expected_revision":app.doc.automation_revision,
            "expected_project_context":app.session.project_context});
        session
            .call(&mut app, "plotium_protein_inspect", query)
            .unwrap()["value"]
            .clone()
    };
    let summary = read(json!({}));
    assert_eq!(summary["molecules"][0]["id"], json!(molecule));
    assert_eq!(summary["molecules"][0]["chain_count"], 1);
    assert!(summary["molecules"][0].get("chains").is_none());
    let chains = read(json!({"view":"chains","molecule":molecule}));
    assert_eq!(chains["chains"][0]["id"], json!(chain));
    let first = read(json!({"view":"residues","molecule":molecule,"chain":chain,"limit":1}));
    assert_eq!(first["next_offset"], 1);
    assert_eq!(first["residues"][0]["number"], 1);
    let last_page =
        read(json!({"view":"residues","molecule":molecule,"chain":chain,"offset":6000,"limit":1}));
    assert_eq!(
        last_page["residues"][0],
        json!({"id":last,"code":"G","number":6001})
    );
    assert!(last_page["next_offset"].is_null());
}
