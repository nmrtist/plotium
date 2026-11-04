use serde_json::{Value, json};
use std::process::Command;

#[test]
fn cli_peak_edits_and_export_match_direct_core_and_persist() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(
        plotium_io::load_path(fixture).unwrap().acquisition,
    )
    .unwrap();
    let mut direct = plotium_core::state::PlotiumApp::new();
    let owner = dataset.resource_id();
    direct.doc.datasets.push(dataset);
    let v = direct.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let ppm: [f64; 3] = std::array::from_fn(|a| v.processed.axis_coordinate(a, 1).unwrap());
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("input.plotium");
    plotium_core::project::save_project(&direct, &project, false).unwrap();
    let id = direct.add_peak_3d(owner, ppm).unwrap();
    let mut requests = vec![json!({"operation":"add","ppm":ppm})];
    direct
        .edit_peak_3d(owner, id, ppm, "manual observation".into())
        .unwrap();
    requests.push(json!({"operation":"edit","id":id,"note":"manual observation"}));
    direct.locate_peak_3d(owner, id).unwrap();
    requests.push(json!({"operation":"locate","id":id}));
    for (index, parameters) in requests.into_iter().enumerate() {
        let request = root.path().join("request.json");
        std::fs::write(
            &request,
            json!({"tool_id":"nmr.peaks3d","parameters":parameters,"targets":plotium_core::automation::TargetSelector::Explicit { ids:vec![owner.to_string()] }}).to_string(),
        )
        .unwrap();
        let output = root.path().join(format!("step-{index}.plotium"));
        let result = Command::new(env!("CARGO_BIN_EXE_plotium-cli"))
            .args(["tool", "run"])
            .arg(&project)
            .arg("--request")
            .arg(&request)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        std::fs::copy(output, &project).unwrap();
    }
    let request = root.path().join("list.json");
    std::fs::write(&request, json!({"tool_id":"nmr.peaks3d.list","targets":plotium_core::automation::TargetSelector::Explicit { ids:vec![owner.to_string()] }}).to_string()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_plotium-cli"))
        .args(["tool", "run"])
        .arg(&project)
        .arg("--request")
        .arg(request)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(result["value"], direct.peak_table_3d(owner).unwrap());
    let reopened = plotium_core::project::load_project(&project).unwrap();
    assert_eq!(
        reopened.peak_table_3d(owner).unwrap(),
        direct.peak_table_3d(owner).unwrap()
    );
}
