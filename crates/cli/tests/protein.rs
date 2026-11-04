use serde_json::{Value, json};
use std::process::Command;

#[test]
fn cli_imports_fasta_and_exports_from_saved_document_without_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input.plotium");
    let output = tmp.path().join("output.plotium");
    let request = tmp.path().join("request.json");
    let mut app = plotium_core::state::PlotiumApp::new();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(
        plotium_io::load_path(fixture).unwrap().acquisition,
    )
    .unwrap();
    let d = dataset.resource_id();
    app.doc.datasets.push(dataset);
    let volume = plotium_core::protein::volume(&app.doc, d).unwrap();
    let ppm = std::array::from_fn(|a| volume.processed.axis_coordinate(a, 1).unwrap());
    let peak = app.add_peak_3d(d, ppm).unwrap();
    plotium_core::project::save_project(&app, &input, false).unwrap();
    let run = |input: &std::path::Path, tool: &str, parameters: Value, save: bool| {
        std::fs::write(
            &request,
            json!({"tool_id":tool,"parameters":parameters}).to_string(),
        )
        .unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_plotium-cli"));
        cmd.args(["tool", "run"])
            .arg(input)
            .arg("--request")
            .arg(&request);
        if save {
            cmd.arg("--output").arg(&output);
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(value.get("error").is_none(), "{value}");
        value
    };
    run(
        &input,
        "protein.edit",
        json!({"operation":"import","source":"two-chains.fasta","fasta":include_str!("../../io/tests/fixtures/protein/two-chains.fasta")}),
        true,
    );
    let reopened = plotium_core::project::load_project(&output).unwrap();
    let molecule = &reopened.doc.protein.molecules[0];
    assert_eq!(molecule.chains.len(), 2);
    let id = molecule.id;
    let residue = molecule.chains[0].residues[0].id;
    let mut direct = reopened;
    use plotium_core::protein::{AssignmentEdit, Edit};
    for (tool, parameters) in [
        (
            "protein.edit",
            json!({"operation":"bind","dataset":d,"molecule":id}),
        ),
        (
            "protein.assignments.edit",
            json!({"dataset":d,"peak":peak,"edit":{"operation":"add","targets":[{"residue":residue,"members":["HB1","HB2"],"group_meaning":"one_of"},null,null]}}),
        ),
    ] {
        std::fs::copy(&output, &input).unwrap();
        run(&input, tool, parameters, true);
    }
    let candidate_state = plotium_core::project::load_project(&output).unwrap();
    let c = candidate_state
        .doc
        .protein
        .assignment(d, peak)
        .unwrap()
        .candidates[0]
        .clone();
    assert_eq!(c.targets[0].as_ref().unwrap().members.len(), 2);
    std::fs::copy(&output, &input).unwrap();
    run(
        &input,
        "protein.assignments.edit",
        json!({"dataset":d,"peak":peak,"edit":{"operation":"confirm","candidate":c.id}}),
        true,
    );
    direct
        .edit_protein(Edit::Bind {
            dataset: d,
            molecule: Some(id),
        })
        .unwrap();
    direct
        .edit_assignment(
            d,
            peak,
            AssignmentEdit::Add {
                targets: c.targets,
                note: String::new(),
            },
        )
        .unwrap();
    let direct_id = direct.doc.protein.assignment(d, peak).unwrap().candidates[0].id;
    direct
        .edit_assignment(
            d,
            peak,
            AssignmentEdit::Confirm {
                candidate: direct_id,
            },
        )
        .unwrap();
    let report = run(
        &output,
        "protein.assignments.export",
        json!({"molecule":id}),
        false,
    );
    let actual = &report["value"]["spectra"][0];
    let expected = &direct.export_protein(id, false).unwrap()["spectra"][0];
    assert_eq!(actual["observations"], expected["observations"]);
    assert_eq!(
        actual["assignments"][0]["candidates"][0]["targets"],
        expected["assignments"][0]["candidates"][0]["targets"]
    );
    assert_eq!(actual["assignments"][0]["confirmed"], json!(c.id));
}
