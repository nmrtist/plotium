use super::*;
#[test]
fn automation_and_direct_edits_share_peak_records_and_readonly_export() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) = crate::workflow::dataset_from_acquisition(
        plotium_io::load_path(&fixture).unwrap().acquisition,
    )
    .unwrap();
    let mut app = crate::state::PlotiumApp::new();
    let id = dataset.resource_id();
    app.doc.datasets.push(dataset);
    let volume = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let ppm = [
        volume.processed.axis_coordinate(0, 1).unwrap(),
        volume.processed.axis_coordinate(1, 1).unwrap(),
        volume.processed.axis_coordinate(2, 2).unwrap(),
    ];
    let run = |app: &mut crate::state::PlotiumApp, tool: &str, parameters: serde_json::Value| {
        let request = ToolRequest {
            tool_id: tool.into(),
            tool_version: 1,
            parameters,
            targets: TargetSelector::Explicit {
                ids: vec![id.to_string()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Agent,
        };
        let plan = plan_tool(app, request).unwrap();
        execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap()
    };
    run(&mut app, PEAKS_3D, json!({"operation":"add","ppm":ppm}));
    let revision = app.doc.automation_revision;
    let result = run(&mut app, PEAKS_3D_LIST, json!({}));
    assert_eq!(revision, app.doc.automation_revision);
    assert!(result.modified.is_empty());
    let mark = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .peaks
        .marks[0]
        .clone();
    assert_eq!(
        result.value["peaks"][0],
        serde_json::to_value(&mark).unwrap()
    );
    app.edit_peak_3d(id, mark.id, ppm, "manual".into()).unwrap();
    let result = run(&mut app, PEAKS_3D_LIST, json!({}));
    assert_eq!(result.value["peaks"][0]["note"], "manual");
    assert_eq!(result.value["axes"].as_array().unwrap().len(), 3);
}
